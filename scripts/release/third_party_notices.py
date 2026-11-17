#!/usr/bin/env python3
"""Generate notices from locked Linux normal/build dependencies, without network access."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def dependency_packages(metadata):
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    root = metadata["resolve"]["root"]
    pending, visited = [root], set()
    while pending:
        identity = pending.pop()
        if identity in visited:
            continue
        visited.add(identity)
        pending.extend(
            dependency["pkg"] for dependency in nodes[identity]["deps"]
            if any(kind["kind"] != "dev" for kind in dependency["dep_kinds"])
        )
    return sorted((packages[i] for i in visited if i != root), key=lambda p: (p["name"], p["version"]))


def license_files(package):
    root = Path(package["manifest_path"]).parent
    files = set()
    # Includes bundled font/icon notices in nested directories, not just crate licenses.
    for path in root.rglob("*"):
        if path.is_file() and path.name.upper().startswith(("LICENSE", "COPYING", "NOTICE", "COPYRIGHT", "OFL")):
            files.add(path)
    if package.get("license_file"):
        path = root / package["license_file"]
        if path.is_file():
            files.add(path)
    override = ROOT / "packaging" / "licenses" / f'{package["name"]}-{package["version"]}'
    if override.is_dir():
        provenance = json.loads((override / "provenance.json").read_text())
        for record in provenance["files"]:
            path = override / record["file"]
            if not path.resolve().is_relative_to(override.resolve()) or hashlib.sha256(path.read_bytes()).hexdigest() != record["sha256"]:
                raise ValueError(f"License provenance mismatch: {path}")
    files.update(path for path in override.glob("*") if path.is_file() and path.name != "provenance.json")
    return sorted(files)


def generate(metadata):
    sections = [
        "Metronomo third-party notices\n\n"
        "Generated from Cargo.lock for x86_64-unknown-linux-gnu. Includes normal and\n"
        "build-time dependencies conservatively; excludes root development-only dependencies.\n"
        "Each component retains its own license. System libraries are provided separately\n"
        "by Ubuntu; see /usr/share/doc/<package>/copyright for those notices.\n"
        "Upstream texts missing from crate archives are pinned under packaging/licenses.\n"
    ]
    missing = []
    for package in dependency_packages(metadata):
        name = f'{package["name"]} {package["version"]}'
        files = license_files(package)
        if not files:
            missing.append(name)
        sections.append(
            f'\n{"=" * 72}\n{name}\nLicense expression: {package.get("license") or "see license file"}\n'
            f'Upstream: {package.get("repository") or package.get("homepage") or "https://crates.io/crates/" + package["name"]}\n'
        )
        root = Path(package["manifest_path"]).parent
        for path in files:
            label = str(path.relative_to(root)) if path.is_relative_to(root) else str(path.relative_to(ROOT))
            sections.append(f'\n--- {label} ---\n{path.read_text(encoding="utf-8")}\n')
    if missing:
        raise ValueError("Missing license texts (packaging blocked): " + ", ".join(missing))
    return "".join(sections)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    metadata = json.loads(subprocess.check_output([
        "cargo", "metadata", "--format-version", "1", "--locked", "--offline",
        "--filter-platform", "x86_64-unknown-linux-gnu",
    ], cwd=ROOT))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(generate(metadata), encoding="utf-8")
    print(f"Wrote {args.output} ({len(dependency_packages(metadata))} components)")


if __name__ == "__main__":
    main()

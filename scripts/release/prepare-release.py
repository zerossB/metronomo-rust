#!/usr/bin/env python3
"""Assemble a reviewable release candidate. Missing evidence keeps ready_to_publish false."""

import argparse
import hashlib
import itertools
import json
from pathlib import Path
import shutil
import subprocess
import tarfile

from source_fingerprint import fingerprint

ROOT = Path(__file__).resolve().parents[2]
SCENARIOS = ("ubuntu-24.04-x11", "ubuntu-24.04-wayland", "ubuntu-26.04-x11", "ubuntu-26.04-wayland")
CHECKS = (
    "ten_minute_desktop_playback", "no_audible_missing_or_duplicate_clicks", "tempo_volume_meter_presets",
    "minimize_restore", "stop_restart", "unavailable_output_retry", "stream_failure_retry",
    "preferences_close_reopen", "clean_install_without_rust", "menu_launch", "reinstall", "remove_preserves_preferences",
)


def valid_renderer_report(name, report):
    cases = report.get("cases", [])
    if name == "renderer-matrix":
        expected = set(itertools.product((44100, 48000, 96000), (64, 127, 256, 511, 1024), (40, 120, 137, 240), (2, 4, 8, 16), (1, 2), (8,)))
    else:
        expected = {(44100, 511, 137, 4, 2, 1800), (48000, 127, 137, 16, 1, 1800), (96000, 1024, 240, 16, 2, 1800)}
    actual = set()
    for case in cases:
        actual.add(tuple(case.get(key) for key in ("sample_rate_hz", "buffer_frames", "bpm", "beat_unit", "channels", "virtual_duration_seconds")))
        error = case.get("maximum_error_frames")
        if (case.get("passed") is not True or isinstance(error, bool) or not isinstance(error, (int, float))
            or not 0 <= error <= 1.01 or case.get("expected_beats", 0) <= 0
            or case.get("rendered_beats") != case.get("expected_beats")
            or case.get("correct_bar_order") is not True or case.get("finite_bounded_samples") is not True):
            return False
    return report.get("passed") is True and actual == expected and len(cases) == len(expected)


def evidence_failures(directory, source_sha256=None, package_sha256=None):
    failures = []
    for name in ("renderer-matrix", "renderer-thirty-minutes"):
        path = directory / f"{name}.json"
        if not path.is_file():
            failures.append(f"Missing {path.name}")
            continue
        report = json.loads(path.read_text())
        if source_sha256 and report.get("environment", {}).get("source_sha256") != source_sha256:
            failures.append(f"Stale source fingerprint: {path.name}")
        if not valid_renderer_report(name, report):
            failures.append(f"Failed or incomplete {path.name}")
    for scenario in SCENARIOS:
        path = directory / scenario / "desktop.json"
        if not path.is_file():
            failures.append(f"Missing {scenario}/desktop.json")
            continue
        report = json.loads(path.read_text())
        if source_sha256 and report.get("source_sha256") != source_sha256:
            failures.append(f"{scenario}: stale source fingerprint")
        if package_sha256 and report.get("package_sha256") != package_sha256:
            failures.append(f"{scenario}: evidence belongs to a different package")
        if report.get("scenario") != scenario or report.get("status") != "passed":
            failures.append(f"Scenario not approved: {scenario}")
        for check in CHECKS:
            if report.get("checks", {}).get(check) is not True:
                failures.append(f"{scenario}: {check} pending/failed")
        for metric, limit in (("maximum_input_to_frame_ms", 100), ("maximum_audio_visual_difference_ms", 50)):
            value = report.get("measurements", {}).get(metric)
            if isinstance(value, bool) or not isinstance(value, (int, float)) or not 0 <= value <= limit:
                failures.append(f"{scenario}: {metric} missing/out of range")
        attachments = report.get("evidence", [])
        base = path.parent.resolve()
        if not attachments:
            failures.append(f"{scenario}: no recordings/measurement evidence")
        for attachment in attachments:
            target = (base / attachment).resolve()
            if not target.is_relative_to(base) or not target.is_file():
                failures.append(f"{scenario}: missing or unsafe evidence path {attachment}")
        audio_path = path.parent / "default-output-ten-minutes.json"
        if not audio_path.is_file():
            failures.append(f"{scenario}: missing real-output diagnostic report")
        else:
            audio = json.loads(audio_path.read_text())
            if source_sha256 and audio.get("environment", {}).get("source_sha256") != source_sha256:
                failures.append(f"{scenario}: stale real-output report")
            diagnostics = audio.get("diagnostics", {})
            if (audio.get("passed") is not True or audio.get("duration_seconds", 0) < 600
                or any(diagnostics.get(key) != 0 for key in ("xruns", "stream_errors", "device_changes", "dropped_visual_events", "dropped_error_events"))):
                failures.append(f"{scenario}: real-output diagnostics failed/incomplete")
    return failures


def package_provenance(package):
    with subprocess.Popen(["dpkg-deb", "--fsys-tarfile", str(package)], stdout=subprocess.PIPE) as process:
        with tarfile.open(fileobj=process.stdout, mode="r|") as archive:
            provenance = None
            for member in archive:
                if member.name == "./usr/share/doc/metronomo/build-provenance.json":
                    provenance = json.load(archive.extractfile(member))
        if process.wait() != 0 or provenance is None:
            raise ValueError("Package has no readable build provenance")
    return provenance


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", required=True, type=Path)
    parser.add_argument("--evidence", type=Path, default=ROOT / "dist" / "evidence")
    parser.add_argument("--require-ready", action="store_true", help="Fail unless every acceptance criterion has evidence")
    args = parser.parse_args()
    package = args.package.resolve()
    fields = subprocess.check_output(["dpkg-deb", "-f", str(package), "Package", "Version", "Architecture"], text=True)
    if fields.splitlines() != ["Package: metronomo", "Version: 0.1.0-1", "Architecture: amd64"]:
        raise SystemExit("Unexpected package identity: " + fields)
    output = ROOT / "dist" / "0.1.0"
    output.mkdir(parents=True, exist_ok=True)
    for source, name in ((package, package.name), (ROOT / "docs/releases/0.1.0.md", "RELEASE-NOTES.md"),
                         (ROOT / "docs/validation/report.md", "VALIDATION.md"), (ROOT / "LICENSE", "LICENSE")):
        shutil.copy2(source, output / name)
    for suffix in ("buildinfo", "changes"):
        sibling = package.with_suffix("." + suffix)
        if sibling.is_file():
            shutil.copy2(sibling, output / sibling.name)
    provenance = package_provenance(package)
    source_sha256 = fingerprint()
    failures = evidence_failures(args.evidence, source_sha256, digest(package))
    if provenance.get("source_sha256") != source_sha256:
        failures.append("Package was built from different Rust inputs")
    if 'VERSION_ID="24.04"' not in provenance.get("os_release", ""):
        failures.append("Package was not built on the Ubuntu 24.04 baseline")
    if args.evidence.is_dir():
        shutil.copytree(args.evidence, output / "evidence", dirs_exist_ok=True)
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    patch = subprocess.check_output(["git", "diff", "--binary", "HEAD"], cwd=ROOT)
    (output / "source.patch").write_bytes(patch)
    paths = subprocess.check_output(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=ROOT).decode().split("\0")
    paths = sorted(set(p for p in paths if p and (p.startswith(("src/", "scripts/", "packaging/", "docs/"))
                       or p in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "README.md", "LICENSE", "CHANGELOG.md", ".gitignore"))))
    source_manifest = "".join(f"{digest(ROOT / p)}  {p}\n" for p in paths if (ROOT / p).is_file())
    (output / "SOURCE-SHA256SUMS").write_text(source_manifest)
    with tarfile.open(output / "SOURCE.tar.gz", mode="w:gz") as archive:
        for path in paths:
            if (ROOT / path).is_file():
                archive.add(ROOT / path, arcname=path, recursive=False)
    manifest = {
        "version": "0.1.0", "package_version": "0.1.0-1", "commit": commit,
        "working_tree_dirty": bool(patch) or bool(subprocess.check_output(["git", "ls-files", "--others", "--exclude-standard"], cwd=ROOT)),
        "source_manifest_sha256": hashlib.sha256(source_manifest.encode()).hexdigest(),
        "source_sha256": source_sha256, "package_build": provenance,
        "package_sha256": digest(package), "ready_to_publish": not failures,
        "pending_acceptance": failures,
    }
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    checksums = "".join(f"{digest(path)}  {path.relative_to(output)}\n" for path in sorted(output.rglob("*")) if path.is_file() and path.name != "SHA256SUMS")
    (output / "SHA256SUMS").write_text(checksums)
    print(f"Prepared {output}; ready_to_publish={not failures}")
    for failure in failures:
        print(f"  Pending: {failure}")
    if args.require_ready and failures:
        raise SystemExit(1)


if __name__ == "__main__":
    main()

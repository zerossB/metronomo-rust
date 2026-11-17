#!/usr/bin/env python3
"""Read and validate the version shared by Cargo, Debian, and GitHub tags."""

import argparse
import json
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parents[2]
VERSION_PATTERN = r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:-[0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?"


def release_metadata(root=ROOT, tag=None):
    version = tomllib.loads((root / "Cargo.toml").read_text())["package"]["version"]
    if not re.fullmatch(VERSION_PATTERN, version):
        raise ValueError(f"Unsupported release version: {version}")
    if tag is not None and tag != f"v{version}":
        raise ValueError(f"Tag {tag!r} does not match Cargo.toml version v{version}")
    changelog = (root / "packaging/linux/debian/changelog").read_text()
    match = re.match(r"metronomo \(([^)]+)\)", changelog)
    expected_upstream = version.replace("-", "~", 1)
    if not match or not re.fullmatch(re.escape(expected_upstream) + r"-[1-9][0-9]*", match[1]):
        raise ValueError(f"Debian changelog must start with metronomo ({expected_upstream}-<revision>)")
    if not (root / "docs/releases" / f"{version}.md").is_file():
        raise ValueError(f"Missing release notes: docs/releases/{version}.md")
    return {
        "version": version,
        "package_version": match[1],
        "package_file": f"metronomo_{match[1]}_amd64.deb",
        "release_dir": f"dist/{version}",
        "prerelease": str("-" in version).lower(),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", help="Require a matching vX.Y.Z or vX.Y.Z-rc.N tag")
    parser.add_argument("--field", choices=("version", "package_version", "package_file", "release_dir", "prerelease"))
    parser.add_argument("--github-output", type=Path)
    args = parser.parse_args()
    try:
        metadata = release_metadata(tag=args.tag)
    except (ValueError, OSError, KeyError) as error:
        parser.error(str(error))
    if args.github_output:
        with args.github_output.open("a") as output:
            output.writelines(f"{key}={value}\n" for key, value in metadata.items())
    print(metadata[args.field] if args.field else json.dumps(metadata, indent=2))


if __name__ == "__main__":
    main()

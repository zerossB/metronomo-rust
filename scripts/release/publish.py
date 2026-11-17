#!/usr/bin/env python3
"""Publish verified assets, resuming drafts or incomplete releases without overwrites."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

from version import VERSION_PATTERN


def digest(path):
    with path.open("rb") as source:
        return "sha256:" + hashlib.file_digest(source, "sha256").hexdigest()


def publication_plan(release, files):
    existing = {asset["name"]: asset for asset in release.get("assets", [])}
    missing = []
    for name, expected in files.items():
        if name not in existing:
            missing.append(name)
        elif existing[name].get("digest") != expected:
            raise ValueError(f"Refusing to replace an existing release asset with different content: {name}")
    if release.get("immutable") and missing:
        raise ValueError("An immutable release is missing assets; publish a new version instead")
    return missing


def read_release(repo, tag):
    result = subprocess.run(["gh", "api", f"repos/{repo}/releases/tags/{tag}"], capture_output=True, text=True)
    data = json.loads(result.stdout)
    if result.returncode:
        if data.get("status") == "404":
            return None
        raise RuntimeError(result.stderr.strip() or data.get("message", "Cannot read release"))
    return data


def publish(repo, tag, assets, notes):
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repo):
        raise ValueError("Expected GitHub repository in owner/name form")
    if not tag.startswith("v") or not re.fullmatch(VERSION_PATTERN, tag[1:]):
        raise ValueError("Expected a vX.Y.Z or vX.Y.Z-rc.N tag")
    paths = {path.name: path for path in sorted(assets.iterdir()) if path.is_file() and not path.is_symlink()}
    if "SHA256SUMS" not in paths or not any(name.endswith(".deb") for name in paths):
        raise ValueError("Expected release assets including a .deb and SHA256SUMS")
    subprocess.run(["sha256sum", "--check", "--strict", "SHA256SUMS"], cwd=assets, check=True)
    files = {name: digest(path) for name, path in paths.items()}
    release = read_release(repo, tag)
    if release:
        for asset in release.get("assets", []):
            if asset["name"] in files and not asset.get("digest"):
                data = subprocess.check_output([
                    "gh", "api", "-H", "Accept: application/octet-stream",
                    f"repos/{repo}/releases/assets/{int(asset['id'])}",
                ])
                asset["digest"] = "sha256:" + hashlib.sha256(data).hexdigest()
    else:
        command = ["gh", "release", "create", tag, "--repo", repo, "--draft", "--verify-tag",
                   "--title", f"Metronomo {tag[1:]}", "--notes-file", str(notes)]
        if "-" in tag:
            command.append("--prerelease")
        subprocess.run(command, check=True)
        release = {"draft": True, "immutable": False, "assets": []}
    missing = publication_plan(release, files)
    if missing:
        subprocess.run(["gh", "release", "upload", tag, "--repo", repo,
                        *[str(paths[name]) for name in missing]], check=True)
    if release.get("draft"):
        subprocess.run(["gh", "release", "edit", tag, "--repo", repo, "--draft=false"], check=True)
    print(f"Release {tag}: {len(missing)} assets uploaded; existing matching assets preserved.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--repo", default=os.environ.get("GH_REPO"))
    parser.add_argument("--assets", type=Path, required=True)
    parser.add_argument("--notes", type=Path, required=True)
    args = parser.parse_args()
    if not args.repo:
        parser.error("Set GH_REPO or pass --repo")
    publish(args.repo, args.tag, args.assets.resolve(), args.notes.resolve())


if __name__ == "__main__":
    main()

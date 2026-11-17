#!/usr/bin/env python3
"""Fingerprint the Rust inputs, independently of documentation/evidence updates."""

import hashlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def fingerprint():
    paths = sorted([ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / "rust-toolchain.toml", *ROOT.glob("src/**/*.rs"), *ROOT.glob("src/**/*.json")])
    digest = hashlib.sha256()
    for path in paths:
        digest.update(path.relative_to(ROOT).as_posix().encode() + b"\0")
        digest.update(hashlib.sha256(path.read_bytes()).digest())
    return digest.hexdigest()


if __name__ == "__main__":
    print(fingerprint())

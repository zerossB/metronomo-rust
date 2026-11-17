#!/usr/bin/env python3
"""Stage only release inputs; dpkg output remains inside this checkout's target/."""

from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[2]
destination = Path(sys.argv[1])
destination.mkdir(parents=True)
for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "README.md", "LICENSE", "CHANGELOG.md"):
    shutil.copy2(ROOT / name, destination / name)
for name in ("src", "scripts", "packaging", "docs"):
    shutil.copytree(ROOT / name, destination / name, ignore=shutil.ignore_patterns("__pycache__"))
(destination / "debian").mkdir()
for name in ("control", "rules", "changelog", "copyright"):
    shutil.copy2(ROOT / "packaging" / "linux" / "debian" / name, destination / "debian" / name)

#!/usr/bin/env python3
"""Record the build inputs inside the installed package."""

import json
from pathlib import Path
import platform
import subprocess
import sys

from source_fingerprint import ROOT, fingerprint

metadata = {
    "source_sha256": fingerprint(),
    "commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
    "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
    "os_release": Path("/etc/os-release").read_text(),
    "architecture": platform.machine(),
    "lockfile_sha256": __import__("hashlib").sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest(),
}
Path(sys.argv[1]).write_text(json.dumps(metadata, indent=2) + "\n")

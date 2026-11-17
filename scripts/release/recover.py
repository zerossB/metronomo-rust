#!/usr/bin/env python3
"""Validate the original successful build before recovering its release artifact."""

import argparse
import json
import os
from pathlib import Path
import re
import subprocess

from version import VERSION_PATTERN


def validate_source(run, jobs, tag, commit):
    if (run["event"] != "push" or run["path"] != ".github/workflows/release.yml"
            or run["head_sha"] != commit or run["head_branch"] != tag):
        raise ValueError("Artifact source must be the original release tag workflow at the expected commit")
    if not any(job["name"] == "Test and build Linux amd64" and job["conclusion"] == "success" for job in jobs):
        raise ValueError("The original release build must have succeeded")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-id", type=int, required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--github-output", type=Path, required=True)
    args = parser.parse_args()
    if not args.tag.startswith("v") or not re.fullmatch(VERSION_PATTERN, args.tag[1:]):
        parser.error("Expected a version tag")
    repo = os.environ["GH_REPO"]
    commit = subprocess.check_output(["git", "rev-parse", f"refs/tags/{args.tag}^{{commit}}"], text=True).strip()
    subprocess.run(["git", "merge-base", "--is-ancestor", commit, "origin/main"], check=True)
    run = json.loads(subprocess.check_output(["gh", "api", f"repos/{repo}/actions/runs/{args.run_id}"]))
    jobs = json.loads(subprocess.check_output(["gh", "api", f"repos/{repo}/actions/runs/{args.run_id}/jobs?filter=latest&per_page=100"]))["jobs"]
    validate_source(run, jobs, args.tag, commit)
    with args.github_output.open("a") as output:
        output.write(f"commit={commit}\nversion={args.tag[1:]}\n")


if __name__ == "__main__":
    main()

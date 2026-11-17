#!/usr/bin/env python3
"""Generate AppStream release details from the Cargo version and Debian changelog."""

import argparse
from email.utils import parsedate_to_datetime
from pathlib import Path
import re
import xml.etree.ElementTree as ET

from version import ROOT, release_metadata


def build_metainfo(root=ROOT):
    metadata = release_metadata(root)
    changelog = (root / "packaging/linux/debian/changelog").read_text()
    first_entry = re.split(r"\n(?=metronomo \()", changelog, maxsplit=1)[0]
    date = re.search(r"^ -- .+?  (.+)$", first_entry, re.MULTILINE)
    if date is None:
        raise ValueError("Debian changelog must include a dated maintainer signature")
    timestamp = parsedate_to_datetime(date[1])
    if timestamp.tzinfo is None:
        raise ValueError("Debian changelog date must include a timezone")
    template = (root / "packaging/linux/desktop/com.haynestec.metronomo.metainfo.xml.in").read_text()
    replacements = {
        "@VERSION@": metadata["version"],
        "@DATE@": timestamp.date().isoformat(),
        "@RELEASE_TYPE@": "development" if metadata["prerelease"] == "true" else "stable",
    }
    for placeholder, value in replacements.items():
        template = template.replace(placeholder, value)
    tree = ET.fromstring(template)
    ET.indent(tree)
    return ET.tostring(tree, encoding="utf-8", xml_declaration=True) + b"\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        contents = build_metainfo()
    except (ValueError, OSError, KeyError, ET.ParseError) as error:
        parser.error(str(error))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(contents)


if __name__ == "__main__":
    main()

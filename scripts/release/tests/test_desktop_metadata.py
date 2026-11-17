from pathlib import Path
import shutil
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from desktop_metadata import build_metainfo
from version import ROOT


class DesktopMetadataTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        (self.root / "packaging/linux/debian").mkdir(parents=True)
        (self.root / "packaging/linux/desktop").mkdir()
        (self.root / "docs/releases").mkdir(parents=True)
        template = Path("packaging/linux/desktop/com.haynestec.metronomo.metainfo.xml.in")
        shutil.copy2(ROOT / template, self.root / template)

    def write_version(self, version="1.2.3"):
        (self.root / "Cargo.toml").write_text(f'[package]\nversion = "{version}"\n')
        (self.root / "docs/releases" / f"{version}.md").write_text(f"# {version}\n")
        self.changelog = self.root / "packaging/linux/debian/changelog"
        debian_version = version.replace("-", "~", 1)
        self.header = f"metronomo ({debian_version}-1) unstable; urgency=medium\n\n  * Release.\n\n"
        self.changelog.write_text(self.header + " -- Maintainer <maintainer@example.org>  Mon, 16 Nov 2026 23:30:00 -0300\n")

    def test_release_details_follow_cargo_and_current_changelog_date(self):
        self.write_version()
        metadata = ET.fromstring(build_metainfo(self.root))
        release = metadata.find("releases/release")
        self.assertEqual((release.attrib, release.findtext("url")), (
            {"version": "1.2.3", "date": "2026-11-16", "type": "stable"},
            "https://github.com/zerossB/metronomo-rust/releases/tag/v1.2.3",
        ))

    def test_prerelease_is_not_advertised_as_a_stable_release(self):
        self.write_version("1.2.3-rc.1")
        release = ET.fromstring(build_metainfo(self.root)).find("releases/release")
        self.assertEqual((release.get("version"), release.get("type")), ("1.2.3-rc.1", "development"))

    def test_incomplete_current_changelog_cannot_borrow_a_previous_release_date(self):
        self.write_version()
        previous = self.changelog.read_text().replace("1.2.3-1", "1.2.2-1")
        self.changelog.write_text(self.header + previous)
        with self.assertRaisesRegex(ValueError, "dated maintainer signature"):
            build_metainfo(self.root)

    def test_ambiguous_date_without_timezone_is_rejected(self):
        self.write_version()
        self.changelog.write_text(self.changelog.read_text().replace(" -0300", ""))
        with self.assertRaisesRegex(ValueError, "timezone"):
            build_metainfo(self.root)


if __name__ == "__main__":
    unittest.main()

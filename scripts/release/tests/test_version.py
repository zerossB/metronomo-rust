from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from version import release_metadata


class ReleaseVersionTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        (self.root / "packaging/linux/debian").mkdir(parents=True)
        (self.root / "docs/releases").mkdir(parents=True)

    def write_version(self, version="1.2.3", package_version="1.2.3-1"):
        (self.root / "Cargo.toml").write_text(f'[package]\nversion = "{version}"\n')
        (self.root / "packaging/linux/debian/changelog").write_text(
            f"metronomo ({package_version}) unstable; urgency=medium\n"
        )
        (self.root / "docs/releases" / f"{version}.md").write_text(f"# Metronomo {version}\n")

    def test_new_versions_determine_package_and_delivery_names(self):
        self.write_version(package_version="1.2.3-2")
        self.assertEqual(release_metadata(self.root, "v1.2.3"), {
            "version": "1.2.3", "package_version": "1.2.3-2",
            "package_file": "metronomo_1.2.3-2_amd64.deb", "release_dir": "dist/1.2.3",
            "prerelease": "false",
        })

    def test_prerelease_uses_debian_ordering_and_github_prerelease_flag(self):
        self.write_version("1.2.3-rc.1", "1.2.3~rc.1-1")
        metadata = release_metadata(self.root, "v1.2.3-rc.1")
        self.assertEqual((metadata["package_file"], metadata["prerelease"]),
                         ("metronomo_1.2.3~rc.1-1_amd64.deb", "true"))

    def test_mismatched_tag_cannot_publish_a_different_version(self):
        self.write_version()
        with self.assertRaisesRegex(ValueError, "does not match"):
            release_metadata(self.root, "v1.2.4")

    def test_stale_debian_changelog_is_rejected_before_building(self):
        self.write_version(package_version="1.2.2-1")
        with self.assertRaisesRegex(ValueError, "Debian changelog"):
            release_metadata(self.root)

    def test_missing_version_specific_notes_are_rejected(self):
        self.write_version()
        (self.root / "docs/releases/1.2.3.md").unlink()
        with self.assertRaisesRegex(ValueError, "Missing release notes"):
            release_metadata(self.root)

    def test_unsafe_version_cannot_reach_shell_or_output_paths(self):
        (self.root / "Cargo.toml").write_text('[package]\nversion = "1.2.3; touch unexpected"\n')
        with self.assertRaisesRegex(ValueError, "Unsupported release version"):
            release_metadata(self.root)


if __name__ == "__main__":
    unittest.main()

from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import publish
from recover import validate_source


class PublishTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.assets = self.root / "assets"
        self.assets.mkdir()
        package = self.assets / "metronomo_1.2.3-1_amd64.deb"
        package.write_bytes(b"test package")
        (self.assets / "SHA256SUMS").write_text(f"{publish.digest(package)[7:]}  {package.name}\n")
        self.notes = self.root / "notes.md"
        self.notes.write_text("Release fixture")
        self.files = {path.name: publish.digest(path) for path in self.assets.iterdir()}

    def run_publication(self, release, fail_upload=False):
        calls = []
        actual_run = subprocess.run

        def run(command, **kwargs):
            if command[0] != "gh":
                return actual_run(command, **kwargs)
            calls.append(command)
            if fail_upload and command[1:3] == ["release", "upload"]:
                raise subprocess.CalledProcessError(1, command)
            return subprocess.CompletedProcess(command, 0)

        with patch.object(publish, "read_release", return_value=release), patch.object(publish.subprocess, "run", side_effect=run):
            if fail_upload:
                with self.assertRaises(subprocess.CalledProcessError):
                    publish.publish("owner/repo", "v1.2.3", self.assets, self.notes)
            else:
                publish.publish("owner/repo", "v1.2.3", self.assets, self.notes)
        return calls

    def test_new_release_uploads_draft_before_publishing(self):
        calls = self.run_publication(None)
        self.assertEqual([call[1:3] for call in calls], [["release", "create"], ["release", "upload"], ["release", "edit"]])
        self.assertIn("--draft", calls[0])

    def test_existing_public_release_gets_missing_assets_without_recreation(self):
        calls = self.run_publication({"draft": False, "immutable": False, "assets": []})
        self.assertEqual([call[1:3] for call in calls], [["release", "upload"]])

    def test_matching_immutable_release_is_a_successful_no_op(self):
        assets = [{"name": name, "digest": value} for name, value in self.files.items()]
        self.assertEqual(self.run_publication({"draft": False, "immutable": True, "assets": assets}), [])

    def test_draft_retry_only_uploads_missing_files(self):
        name = next(name for name in self.files if name.endswith(".deb"))
        calls = self.run_publication({"draft": True, "immutable": False, "assets": [{"name": name, "digest": self.files[name]}]})
        self.assertNotIn(str(self.assets / name), calls[0])
        self.assertEqual(calls[-1][1:3], ["release", "edit"])

    def test_failed_upload_does_not_publish_draft(self):
        calls = self.run_publication({"draft": True, "immutable": False, "assets": []}, fail_upload=True)
        self.assertEqual([call[1:3] for call in calls], [["release", "upload"]])

    def test_conflicting_asset_is_never_replaced(self):
        with self.assertRaisesRegex(ValueError, "Refusing to replace"):
            publish.publication_plan({"assets": [{"name": "example.deb", "digest": "sha256:old"}]}, {"example.deb": "sha256:new"})

    def test_missing_asset_cannot_be_added_to_immutable_release(self):
        with self.assertRaisesRegex(ValueError, "immutable release"):
            publish.publication_plan({"immutable": True, "assets": []}, self.files)


class RecoveryTests(unittest.TestCase):
    def setUp(self):
        self.source_run = {"event": "push", "path": ".github/workflows/release.yml", "head_sha": "expected", "head_branch": "v1.2.3"}
        self.jobs = [{"name": "Test and build Linux amd64", "conclusion": "success"}]

    def test_successful_build_can_be_recovered_after_publication_failed(self):
        validate_source(self.source_run, self.jobs, "v1.2.3", "expected")

    def test_pr_artifacts_cannot_be_used_to_publish(self):
        self.source_run["event"] = "pull_request"
        with self.assertRaisesRegex(ValueError, "original release tag"):
            validate_source(self.source_run, self.jobs, "v1.2.3", "expected")

    def test_artifact_for_another_commit_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "original release tag"):
            validate_source(self.source_run, self.jobs, "v1.2.3", "another")

    def test_failed_build_is_rejected(self):
        self.jobs[0]["conclusion"] = "failure"
        with self.assertRaisesRegex(ValueError, "build must have succeeded"):
            validate_source(self.source_run, self.jobs, "v1.2.3", "expected")


if __name__ == "__main__":
    unittest.main()

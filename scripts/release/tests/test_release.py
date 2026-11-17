import importlib.util
import itertools
import json
from pathlib import Path
import sys
import tempfile
import unittest

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


release = load("release", "prepare-release.py")
notices = load("notices", "third_party_notices.py")


class ReleaseEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)

    def complete_evidence(self):
        for name in ("renderer-matrix", "renderer-thirty-minutes"):
            configurations = itertools.product((44100, 48000, 96000), (64, 127, 256, 511, 1024), (40, 120, 137, 240), (2, 4, 8, 16), (1, 2), (8,)) if name == "renderer-matrix" else ((44100, 511, 137, 4, 2, 1800), (48000, 127, 137, 16, 1, 1800), (96000, 1024, 240, 16, 2, 1800))
            cases = [dict(zip(("sample_rate_hz", "buffer_frames", "bpm", "beat_unit", "channels", "virtual_duration_seconds"), configuration),
                          passed=True, maximum_error_frames=0.9, expected_beats=10, rendered_beats=10,
                          correct_bar_order=True, finite_bounded_samples=True) for configuration in configurations]
            (self.directory / f"{name}.json").write_text(json.dumps({
                "passed": True, "environment": {"source_sha256": "current"}, "cases": cases,
            }))
        for scenario in release.SCENARIOS:
            directory = self.directory / scenario
            directory.mkdir()
            (directory / "capture.txt").write_text("Test fixture only; never release evidence.")
            (directory / "desktop.json").write_text(json.dumps({
                "scenario": scenario, "status": "passed", "source_sha256": "current",
                "checks": dict.fromkeys(release.CHECKS, True),
                "measurements": {"maximum_input_to_frame_ms": 90, "maximum_audio_visual_difference_ms": 45},
                "evidence": ["capture.txt"],
            }))
            (directory / "default-output-ten-minutes.json").write_text(json.dumps({
                "passed": True, "environment": {"source_sha256": "current"}, "duration_seconds": 600,
                "diagnostics": dict.fromkeys(("xruns", "stream_errors", "device_changes", "dropped_visual_events", "dropped_error_events"), 0),
            }))

    def change_desktop(self, transform):
        path = self.directory / release.SCENARIOS[0] / "desktop.json"
        report = json.loads(path.read_text())
        transform(report)
        path.write_text(json.dumps(report))

    def test_missing_platforms_and_measurements_never_approve_release(self):
        self.assertEqual(len(release.evidence_failures(self.directory, "current")), 6)
        self.complete_evidence()
        self.change_desktop(lambda report: report["measurements"].pop("maximum_audio_visual_difference_ms"))
        self.assertTrue(any("maximum_audio_visual_difference_ms" in error for error in release.evidence_failures(self.directory, "current")))

    def test_stale_source_reports_never_approve_release(self):
        self.complete_evidence()
        self.assertEqual(release.evidence_failures(self.directory, "current"), [])
        self.assertTrue(release.evidence_failures(self.directory, "different-source"))

    def test_a_truncated_matrix_cannot_be_approved_by_its_passed_flag(self):
        self.complete_evidence()
        path = self.directory / "renderer-matrix.json"
        report = json.loads(path.read_text())
        report["cases"].pop()
        path.write_text(json.dumps(report))
        self.assertTrue(any("incomplete renderer-matrix" in error for error in release.evidence_failures(self.directory, "current")))

    def test_evidence_must_describe_the_same_package(self):
        self.complete_evidence()
        self.assertTrue(any("different package" in error for error in release.evidence_failures(self.directory, "current", "new-package")))

    def test_boolean_measurements_are_not_treated_as_milliseconds(self):
        self.complete_evidence()
        self.change_desktop(lambda report: report["measurements"].update(maximum_input_to_frame_ms=True))
        self.assertTrue(release.evidence_failures(self.directory, "current"))

    def test_missing_or_external_recording_never_approves_release(self):
        self.complete_evidence()
        self.change_desktop(lambda report: report.update(evidence=["../../outside.mp4"]))
        self.assertTrue(any("unsafe evidence" in error for error in release.evidence_failures(self.directory, "current")))


class NoticesTests(unittest.TestCase):
    def test_dev_only_dependencies_are_excluded_but_transitive_build_dependencies_remain(self):
        def edge(package, kind=None):
            return {"pkg": package, "dep_kinds": [{"kind": kind}]}
        metadata = {
            "packages": [{"id": name, "name": name, "version": "1"} for name in ("root", "normal", "builder", "dev-only")],
            "resolve": {"root": "root", "nodes": [
                {"id": "root", "deps": [edge("normal"), edge("dev-only", "dev")]},
                {"id": "normal", "deps": [edge("builder", "build")]},
                {"id": "builder", "deps": []}, {"id": "dev-only", "deps": []},
            ]},
        }
        self.assertEqual([p["name"] for p in notices.dependency_packages(metadata)], ["builder", "normal"])


if __name__ == "__main__":
    unittest.main()

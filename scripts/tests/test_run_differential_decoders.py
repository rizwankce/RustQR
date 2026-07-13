import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "run_differential_decoders.py"
CONFORMANCE_ROOT = Path(__file__).parents[2] / "conformance"
SPEC = importlib.util.spec_from_file_location("run_differential_decoders", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class DifferentialDecoderTests(unittest.TestCase):
    def write_manifest(self, root: Path, schema="rustqr.conformance-manifest.v1") -> Path:
        path = root / "manifest.json"
        path.write_text(
            json.dumps(
                {
                    "schema_version": schema,
                    "cases": [
                        {
                            "id": "fixture-1",
                            "expected": {"raw_payload_hex": "4142"},
                            "matrix": {"path": None, "status": "planned"},
                        }
                    ],
                }
            ),
            encoding="utf-8",
        )
        return path

    def test_missing_matrix_is_reported_without_invoking_decoder(self):
        with tempfile.TemporaryDirectory() as directory:
            report = MODULE.run(self.write_manifest(Path(directory)), ["zbar", "opencv"])
        self.assertEqual(report["schema_version"], "rustqr.differential-report.v1")
        self.assertEqual(report["cases"][0]["decoders"]["zbar"]["status"], "not_generated")
        self.assertEqual(
            report["cases"][0]["decoders"]["opencv"]["status"], "not_generated"
        )
        self.assertEqual(report["corpus"]["matrix_status_counts"], {"planned": 1})

    def test_incompatible_manifest_schema_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = self.write_manifest(Path(directory), "old-schema")
            with self.assertRaisesRegex(ValueError, "rustqr.conformance-manifest.v1"):
                MODULE.run(path, ["zbar"])

    def test_invalid_fixture_rejection_is_distinct_from_decode_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "invalid.png").write_bytes(b"not needed by fake adapter")
            path = root / "manifest.json"
            path.write_text(
                json.dumps(
                    {
                        "schema_version": "rustqr.conformance-manifest.v1",
                        "cases": [
                            {
                                "id": "invalid-format",
                                "expected": {"raw_payload_hex": "", "outcome": "reject"},
                                "matrix": {"path": "invalid.png"},
                            }
                        ],
                    }
                )
            )
            original = MODULE.ADAPTERS["zbar"]
            MODULE.ADAPTERS["zbar"] = lambda _: ("decode_error", None, "rejected")
            try:
                report = MODULE.run(path, ["zbar"])
            finally:
                MODULE.ADAPTERS["zbar"] = original
        self.assertEqual(
            report["cases"][0]["decoders"]["zbar"]["status"], "expected_rejection"
        )

    def test_only_text_adapters_reject_arbitrary_binary_fidelity(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "binary.png").write_bytes(b"not invoked")
            path = root / "manifest.json"
            path.write_text(
                json.dumps(
                    {
                        "schema_version": "rustqr.conformance-manifest.v1",
                        "cases": [
                            {
                                "id": "binary",
                                "expected": {"raw_payload_hex": "ff00"},
                                "matrix": {"path": "binary.png"},
                            }
                        ],
                    }
                )
            )
            original = MODULE.ADAPTERS["zbar"]
            MODULE.ADAPTERS["zbar"] = lambda _: ("decoded", b"\xff\x00", None)
            try:
                zbar_report = MODULE.run(path, ["zbar"])
            finally:
                MODULE.ADAPTERS["zbar"] = original
            opencv_report = MODULE.run(path, ["opencv"])

            self.assertEqual(
                zbar_report["cases"][0]["decoders"]["zbar"]["status"],
                "match",
            )
            self.assertEqual(
                opencv_report["cases"][0]["decoders"]["opencv"]["status"],
                "unsupported",
            )

    def test_generated_matrix_checksum_is_verified_before_decode(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "matrix.png").write_bytes(b"matrix")
            path = root / "manifest.json"
            path.write_text(
                json.dumps(
                    {
                        "schema_version": "rustqr.conformance-manifest.v1",
                        "cases": [
                            {
                                "id": "corrupt",
                                "expected": {"raw_payload_hex": "41"},
                                "matrix": {
                                    "path": "matrix.png",
                                    "status": "generated",
                                    "sha256": "0" * 64,
                                },
                            }
                        ],
                    }
                )
            )
            report = MODULE.run(path, ["zbar"])
        self.assertEqual(
            report["cases"][0]["decoders"]["zbar"]["status"], "invalid_fixture"
        )
        self.assertEqual(report["summary"]["zbar"]["generated_total"], 1)

    def test_explicitly_unsupported_mode_is_accounted_separately(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "manifest.json"
            path.write_text(
                json.dumps(
                    {
                        "schema_version": "rustqr.conformance-manifest.v1",
                        "cases": [
                            {
                                "id": "kanji",
                                "mode": "kanji",
                                "expected": {"raw_payload_hex": ""},
                                "matrix": {"path": None, "status": "unsupported_mode"},
                            }
                        ],
                    }
                )
            )
            report = MODULE.run(path, ["zbar"])
        self.assertEqual(report["cases"][0]["decoders"]["zbar"]["status"], "unsupported")
        self.assertEqual(
            report["corpus"]["matrix_status_counts"], {"unsupported_mode": 1}
        )

    def test_kanji_text_reencoding_is_not_reported_as_raw_byte_mismatch(self):
        status, detail = MODULE.compare_payload(
            "zbar", "kanji", "漢字".encode("shift_jis"), "漢字".encode("utf-8")
        )
        self.assertEqual(status, "text_match")
        self.assertIn("Shift-JIS", detail)

    def test_recorded_evidence_tracks_each_adapter(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = self.write_manifest(root)
            report = MODULE.run(path, ["zbar"])
            MODULE.record_manifest_evidence(path, report, root / "report.json")
            manifest = json.loads(path.read_text(encoding="utf-8"))
        evidence = manifest["cases"][0]["differential"]
        self.assertEqual(evidence["status"], "recorded")
        self.assertEqual(evidence["report"], "report.json")
        self.assertEqual(evidence["decoders"]["zbar"]["status"], "not_generated")

    def test_checked_in_report_and_manifest_evidence_agree(self):
        manifest_path = CONFORMANCE_ROOT / "manifest.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        report = json.loads(
            (CONFORMANCE_ROOT / "differential-report.json").read_text(encoding="utf-8")
        )
        self.assertEqual(
            report["manifest"]["sha256"],
            MODULE.hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
        )
        rows = {row["id"]: row for row in report["cases"]}
        for case in manifest["cases"]:
            evidence = case["differential"]
            self.assertEqual(evidence["status"], "recorded")
            self.assertEqual(evidence["report"], "differential-report.json")
            for adapter, adapter_evidence in evidence["decoders"].items():
                self.assertEqual(
                    adapter_evidence,
                    {
                        key: rows[case["id"]]["decoders"][adapter][key]
                        for key in ("status", "raw_payload_hex")
                    },
                )


if __name__ == "__main__":
    unittest.main()

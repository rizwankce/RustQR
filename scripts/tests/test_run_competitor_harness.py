import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "run_competitor_harness.py"
SPEC = importlib.util.spec_from_file_location("competitor_harness", SCRIPT)
assert SPEC and SPEC.loader
HARNESS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(HARNESS)


class CompetitorHarnessTests(unittest.TestCase):
    def test_annotation_count_requires_quadrilaterals(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "case.txt"
            path.write_text("# comment\n1 1\n2 2\n3 3\n4 4\n")
            self.assertEqual(HARNESS.expected_symbols(path), 1)
            path.write_text("1 1\n2 2\n")
            with self.assertRaises(ValueError):
                HARNESS.expected_symbols(path)

    def test_summary_excludes_unavailable_from_coverage(self):
        rows = [
            {"expected_symbols": 2, "result": {"status": "decoded", "payloads_hex": ["61"], "latency_ms": 4.0}},
            {"expected_symbols": 4, "result": {"status": "unavailable", "payloads_hex": [], "latency_ms": None}},
        ]
        summary = HARNESS.summarize(rows)
        self.assertEqual(summary["expected_annotation_symbols"], 2)
        self.assertEqual(summary["annotation_count_coverage"], 0.5)
        self.assertEqual(summary["latency_ms"]["median"], 4.0)

    def test_case_collection_is_sorted_and_bounded(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "images"
            category = root / "nominal"
            category.mkdir(parents=True)
            for name in ("b.jpg", "a.jpg"):
                image = category / name
                image.write_bytes(name.encode())
                image.with_suffix(".txt").write_text("1 1\n2 2\n3 3\n4 4\n")
            cases = HARNESS.collect_cases(root, "nominal", 1)
            self.assertEqual([case["id"] for case in cases], ["nominal/a.jpg"])

    def test_lock_is_json_and_has_every_adapter(self):
        lock = json.loads((SCRIPT.parents[1] / "competitors/lock.json").read_text())
        self.assertEqual(lock["schema_version"], "rustqr.competitor-lock.v1")
        self.assertEqual(set(lock["adapters"]), set(HARNESS.ADAPTERS))

    def test_setup_reports_runner_state(self):
        setup = HARNESS.adapter_setup("quirc")
        self.assertIn(setup["status"], {"available", "missing"})
        if setup["status"] == "available":
            self.assertIn("command", setup)
        else:
            self.assertIn("quirc_decode", setup["detail"])

    def test_thread_environment_is_explicitly_single_threaded(self):
        self.assertEqual(HARNESS.THREAD_ENVIRONMENT["OMP_NUM_THREADS"], "1")
        self.assertEqual(HARNESS.THREAD_ENVIRONMENT["OPENBLAS_NUM_THREADS"], "1")


if __name__ == "__main__":
    unittest.main()

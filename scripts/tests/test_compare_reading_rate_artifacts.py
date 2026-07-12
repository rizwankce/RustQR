import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "compare_reading_rate_artifacts.py"
SPEC = importlib.util.spec_from_file_location("compare_reading_rate_artifacts", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class CompatibilityTests(unittest.TestCase):
    def artifact(self):
        return {
            "metadata": {
                "label_fingerprint": "labels",
                "evaluator_fingerprint": "evaluator",
                "preprocessing_fingerprint": "preprocessing",
                "limit_per_category": 3,
                "smoke": False,
                "selected_category": "nominal",
            },
            "categories": [{"name": "nominal"}],
        }

    def test_reads_complete_comparison_scope(self):
        result = MODULE.read_compatibility(self.artifact(), Path("candidate.json"))
        self.assertEqual(result["evaluated_categories"], ["nominal"])
        self.assertEqual(result["preprocessing_fingerprint"], "preprocessing")

    def test_rejects_missing_evaluator_fingerprint(self):
        artifact = self.artifact()
        del artifact["metadata"]["evaluator_fingerprint"]
        with self.assertRaisesRegex(ValueError, "evaluator_fingerprint"):
            MODULE.read_compatibility(artifact, Path("candidate.json"))

    def test_rejects_historical_schema(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "old.json"
            path.write_text(json.dumps({"schema_version": "wp007-reading-rate-v1", "summary": {}}))
            with self.assertRaisesRegex(ValueError, "incompatible schema_version"):
                MODULE.load_artifact(path)


if __name__ == "__main__":
    unittest.main()

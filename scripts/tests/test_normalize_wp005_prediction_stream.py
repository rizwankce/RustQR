"""Contract tests for the throwaway WP-005 prediction-stream normalizer."""

from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "normalize_wp005_prediction_stream.py"
SPEC = importlib.util.spec_from_file_location("wp005_normalizer", SCRIPT)
assert SPEC and SPEC.loader
NORMALIZER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(NORMALIZER)


def square(x: float, y: float, side: float = 10.0) -> list[list[float]]:
    return [[x, y], [x + side, y], [x + side, y + side], [x, y + side]]


class PredictionStreamNormalizerTests(unittest.TestCase):
    def test_normalizes_maximum_matching_duplicates_and_timeouts(self) -> None:
        truth = {
            "schema_version": NORMALIZER.TRUTH_SCHEMA,
            "metadata": {"dataset_fingerprint": "pixels", "label_fingerprint": "labels", "preprocessing_fingerprint": "rgb8;triangle-resize;max-dim=1024"},
            "images": [
                {"image_id": "nominal/a.jpg", "category": "nominal", "original_width": 100, "original_height": 100, "working_width": 100, "working_height": 100, "expected_quadrilaterals": [square(0, 0)]},
                {"image_id": "rotations/b.jpg", "category": "rotations", "original_width": 100, "original_height": 100, "working_width": 100, "working_height": 100, "expected_quadrilaterals": [square(20, 20)]},
            ],
        }
        predictions = {
            "schema_version": NORMALIZER.PREDICTION_SCHEMA,
            "metadata": {"dataset_fingerprint": "pixels", "preprocessing_fingerprint": "rgb8;triangle-resize;max-dim=1024", "commit_sha": "candidate", "limit_per_category": 25},
            "images": [
                {"image_id": "nominal/a.jpg", "category": "nominal", "original_width": 100, "original_height": 100, "working_width": 100, "working_height": 100, "predicted_quadrilaterals": [square(0, 0), square(0, 0), square(70, 70)], "core_elapsed_ms": 1, "end_to_end_elapsed_ms": 2, "timed_out": False},
                {"image_id": "rotations/b.jpg", "category": "rotations", "original_width": 100, "original_height": 100, "working_width": 100, "working_height": 100, "predicted_quadrilaterals": [square(20, 20)], "core_elapsed_ms": 3, "end_to_end_elapsed_ms": 4, "timed_out": True},
            ],
        }
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            prediction_path, truth_path, output_path = root / "predictions.json", root / "truth.json", root / "out.json"
            prediction_path.write_text(json.dumps(predictions), encoding="utf-8")
            truth_path.write_text(json.dumps(truth), encoding="utf-8")
            self.assertEqual(NORMALIZER.main.__name__, "main")
            import subprocess
            completed = subprocess.run(["python3", str(SCRIPT), "--predictions", str(prediction_path), "--truth", str(truth_path), "--output", str(output_path)], text=True, capture_output=True, check=False)
            self.assertEqual(completed.returncode, 0, completed.stderr)
            result = json.loads(output_path.read_text(encoding="utf-8"))
            comparator = subprocess.run(
                ["python3", str(ROOT / "scripts" / "compare_reading_rate_artifacts.py"),
                 "--baseline", str(output_path), "--candidate", str(output_path),
                 "--category-max-drop-pp", "nominal=0", "--category-max-drop-pp", "rotations=0"],
                text=True, capture_output=True, check=False,
            )
            self.assertEqual(comparator.returncode, 0, comparator.stderr)
        self.assertEqual(result["schema_version"], "rustqr.reading_rate.v2")
        self.assertEqual(result["summary"]["total_hits"], 1)
        self.assertEqual(result["summary"]["false_positives"], 2)
        self.assertEqual(result["summary"]["duplicate_predictions"], 1)
        self.assertEqual(result["summary"]["timeouts"], 1)
        self.assertEqual(result["summary"]["false_negatives"], 1)

    def test_rejects_dimension_mismatch(self) -> None:
        truth = {"schema_version": NORMALIZER.TRUTH_SCHEMA, "metadata": {"dataset_fingerprint": "pixels", "label_fingerprint": "labels", "preprocessing_fingerprint": "p"}, "images": [{"image_id": "a", "category": "nominal", "original_width": 10, "original_height": 10, "working_width": 10, "working_height": 10, "expected_quadrilaterals": []}]}
        predictions = {"schema_version": NORMALIZER.PREDICTION_SCHEMA, "metadata": {"dataset_fingerprint": "pixels", "preprocessing_fingerprint": "p", "commit_sha": "x"}, "images": [{"image_id": "a", "category": "nominal", "original_width": 9, "original_height": 10, "working_width": 10, "working_height": 10, "predicted_quadrilaterals": [], "core_elapsed_ms": 0, "end_to_end_elapsed_ms": 0, "timed_out": False}]}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            paths = [root / name for name in ("predictions.json", "truth.json", "out.json")]
            paths[0].write_text(json.dumps(predictions), encoding="utf-8")
            paths[1].write_text(json.dumps(truth), encoding="utf-8")
            import subprocess
            completed = subprocess.run(["python3", str(SCRIPT), "--predictions", str(paths[0]), "--truth", str(paths[1]), "--output", str(paths[2])], text=True, capture_output=True, check=False)
            self.assertEqual(completed.returncode, 2)
            self.assertIn("original_width", completed.stderr)


if __name__ == "__main__":
    unittest.main()

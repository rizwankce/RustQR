"""Regression coverage for the deterministic WP-012 raster corpus builder."""

from __future__ import annotations

import json
import importlib.util
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/generate_wp012_raster_scenes.py"
EVALUATOR = ROOT / "scripts/evaluate_wp012_raster_scenes.py"


class GenerateWp012RasterScenesTests(unittest.TestCase):
    def test_generates_all_density_steps_with_strict_labels(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "controlled_dense"
            subprocess.run([sys.executable, str(SCRIPT), "--output", str(output)], check=True)
            manifest = json.loads((output.parent / "manifest.json").read_text())
            self.assertEqual([scene["symbols"] for scene in manifest["scenes"]], [1, 2, 5, 10, 25, 50, 100])
            for scene in manifest["scenes"]:
                stem = scene["id"]
                lines = (output / f"{stem}.txt").read_text().splitlines()
                self.assertEqual(lines[1], "SETS")
                self.assertEqual(len(lines) - 2, scene["symbols"])
                self.assertTrue((output / f"{stem}.png").is_file())

    def test_density_evaluator_preserves_bipartite_fields(self) -> None:
        spec = importlib.util.spec_from_file_location("wp012_evaluator", EVALUATOR)
        assert spec and spec.loader
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        row = module.summarize(
            {
                "metadata": {"evaluator_fingerprint": "mode=localization;quad-iou=0.5;matching=max-cardinality"},
                "summary": {
                    "total_hits": 2,
                    "total_expected": 5,
                    "localization_recall": 0.4,
                    "localization_precision": 1.0,
                    "false_positives": 0,
                    "duplicate_predictions": 1,
                    "timeouts": 0,
                    "qr_symbols_per_second": 12.5,
                    "end_to_end_runtime": {"mean_per_image_ms": 400.0},
                    "core_runtime": {"mean_per_image_ms": 390.0},
                },
            },
            5,
        )
        self.assertEqual((row["symbols"], row["hits"], row["duplicates"]), (5, 2, 1))
        self.assertEqual(row["evaluator_fingerprint"], "mode=localization;quad-iou=0.5;matching=max-cardinality")


if __name__ == "__main__":
    unittest.main()

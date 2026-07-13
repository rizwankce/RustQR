import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "evaluate_negative_corpus.py"
SPEC = importlib.util.spec_from_file_location("evaluate_negative_corpus", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class ParseMetricsTests(unittest.TestCase):
    def test_parses_complete_metric_line(self):
        output = (
            "NEGATIVE_CORPUS_METRICS cases=9 pixels=589824 megapixels=0.589824 "
            "positive_images=0 false_positive_detections=0 fp_per_image=0.000000 "
            "fp_per_megapixel=0.000000\n"
        )
        self.assertEqual(
            MODULE.parse_metrics(output),
            {
                "cases": 9,
                "pixels": 589824,
                "megapixels": 0.589824,
                "positive_images": 0,
                "false_positive_detections": 0,
                "false_positives_per_image": 0.0,
                "false_positives_per_megapixel": 0.0,
            },
        )

    def test_rejects_missing_metric_line(self):
        with self.assertRaisesRegex(ValueError, "NEGATIVE_CORPUS_METRICS"):
            MODULE.parse_metrics("test result: ok")


if __name__ == "__main__":
    unittest.main()

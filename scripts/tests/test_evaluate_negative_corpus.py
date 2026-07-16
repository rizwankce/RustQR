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
            "positive_images=0 timeout_images=0 false_positive_detections=0 fp_per_image=0.000000 "
            "fp_per_megapixel=0.000000\n"
            "ZXING_NEGATIVE_CORPUS_METRICS cases=47 pixels=12518400 megapixels=12.518400 "
            "positive_images=0 timeout_images=0 false_positive_detections=0 fp_per_image=0.000000 "
            "fp_per_megapixel=0.000000\n"
            "ZXING_CROSS_SYMBOLOGY_NEGATIVE_CORPUS_METRICS cases=47 pixels=5068674 megapixels=5.068674 "
            "positive_images=0 timeout_images=0 false_positive_detections=0 fp_per_image=0.000000 "
            "fp_per_megapixel=0.000000\n"
            "WIKIMEDIA_NEGATIVE_CORPUS_METRICS cases=3 pixels=26839032 megapixels=26.839032 "
            "positive_images=0 timeout_images=0 false_positive_detections=0 fp_per_image=0.000000 "
            "fp_per_megapixel=0.000000\n"
        )
        reports = MODULE.parse_metrics(
            output,
            {
                "negative_corpus",
                "zxing_negative_corpus",
                "zxing_cross_symbology_negative_corpus",
                "wikimedia_negative_corpus",
            },
        )
        self.assertEqual(
            set(reports),
            {
                "negative_corpus",
                "zxing_negative_corpus",
                "zxing_cross_symbology_negative_corpus",
                "wikimedia_negative_corpus",
            },
        )
        self.assertEqual(reports["negative_corpus"]["cases"], 9)
        self.assertEqual(reports["zxing_negative_corpus"]["cases"], 47)
        self.assertEqual(
            reports["zxing_cross_symbology_negative_corpus"]["cases"], 47
        )
        self.assertEqual(reports["wikimedia_negative_corpus"]["cases"], 3)

    def test_rejects_missing_metric_line(self):
        with self.assertRaisesRegex(ValueError, "expected"):
            MODULE.parse_metrics("test result: ok", {"negative_corpus"})


if __name__ == "__main__":
    unittest.main()

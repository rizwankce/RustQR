import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "verify_wp009_wikimedia_corpus.py"
SPEC = importlib.util.spec_from_file_location("verify_wp009_wikimedia_corpus", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class WikimediaCorpusVerifierTests(unittest.TestCase):
    def test_checked_in_multi_asset_manifest(self):
        cases, pixels = MODULE.verify_manifest()
        self.assertEqual(cases, 3)
        self.assertEqual(pixels, 26_839_032)

    def test_rejects_unknown_image_type(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "fixture.gif"
            path.write_bytes(b"not-an-image")
            with self.assertRaisesRegex(ValueError, "unsupported"):
                MODULE.image_dimensions(path)


if __name__ == "__main__":
    unittest.main()

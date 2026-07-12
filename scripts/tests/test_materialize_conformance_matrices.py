import importlib.util
import hashlib
import json
import tempfile
import unittest
from pathlib import Path


GENERATOR_PATH = Path(__file__).parents[1] / "generate_conformance_manifest.py"
MATERIALIZER_PATH = Path(__file__).parents[1] / "materialize_conformance_matrices.py"


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


GENERATOR = load(GENERATOR_PATH, "generator")
MATERIALIZER = load(MATERIALIZER_PATH, "materializer")


class MaterializerTests(unittest.TestCase):
    def test_materializes_supported_case_and_marks_unsupported(self):
        manifest = GENERATOR.generate_manifest()
        manifest["cases"] = [
            next(case for case in manifest["cases"] if case["mode"] == "byte"),
            next(case for case in manifest["cases"] if case["mode"] == "eci"),
        ]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "manifest.json"
            path.write_text(json.dumps(manifest), encoding="utf-8")
            result = MATERIALIZER.materialize(path)
            generated, unsupported = result["cases"]
            self.assertEqual(generated["matrix"]["status"], "generated")
            matrix = path.parent / generated["matrix"]["path"]
            self.assertTrue(matrix.is_file())
            self.assertEqual(
                generated["matrix"]["sha256"],
                hashlib.sha256(matrix.read_bytes()).hexdigest(),
            )
            self.assertEqual(generated["matrix"]["generator_backend"], MATERIALIZER.BACKEND)
            self.assertEqual(unsupported["matrix"]["status"], "unsupported_mode")
            self.assertGreater(result["capacity_validation"][0]["maximum_units"], 0)
            self.assertTrue(result["capacity_validation"][0]["maximum_plus_one_rejected"])


if __name__ == "__main__":
    unittest.main()

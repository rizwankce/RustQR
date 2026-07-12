import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "generate_conformance_manifest.py"
SPEC = importlib.util.spec_from_file_location("generate_conformance_manifest", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class ManifestTests(unittest.TestCase):
    def test_foundation_manifest_is_deterministic(self):
        manifest = MODULE.generate_manifest()
        self.assertEqual(manifest, MODULE.generate_manifest())
        self.assertLess(len(manifest["cases"]), 50)
        self.assertEqual({case["mask"] for case in manifest["cases"]}, set(range(8)))
        self.assertEqual({case["mode"] for case in manifest["cases"]}, set(MODULE.MODES))

    def test_full_profile_scaffolds_model2_cross_product(self):
        manifest = MODULE.generate_manifest(full=True)
        coverage = manifest["coverage"]
        self.assertEqual(coverage["versions"], list(range(1, 41)))
        self.assertEqual(set(coverage["ec_levels"]), {"L", "M", "Q", "H"})
        self.assertEqual(set(coverage["masks"]), set(range(8)))
        expected_count = 40 * 4 * 8 * len(MODULE.MODES)
        self.assertEqual(len(manifest["cases"]), expected_count)
        self.assertEqual(
            sum(case["mode"] in {"numeric", "alphanumeric", "byte"} for case in manifest["cases"]),
            40 * 4 * 8 * 3,
        )

    def test_case_records_preserve_raw_payload_and_metadata(self):
        case = MODULE.generate_manifest()["cases"][0]
        self.assertEqual(case["payload_hex"], case["expected"]["raw_payload_hex"])
        self.assertEqual(case["version"], case["expected"]["version"])
        self.assertEqual(case["matrix"]["status"], "planned")


if __name__ == "__main__":
    unittest.main()

import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "plan_conformance_mutations.py"
SPEC = importlib.util.spec_from_file_location("plan_conformance_mutations", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class MutationPlanTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        root = Path(__file__).parents[2]
        cls.plan = MODULE.generate(
            root / "conformance/manifest.json", root / "src/decoder/tables.rs"
        )

    def test_plans_are_reproducible_and_use_rs_limits(self):
        mutations = self.plan["mutations"]
        case = next(row for row in mutations if row["id"] == "v01-H-m0-byte--correctable_errors")
        self.assertEqual(case["mutation"]["parameters"]["ecc_codewords_per_block"], 17)
        self.assertEqual(case["mutation"]["parameters"]["errors_per_block"], 8)
        self.assertEqual(case["mutation"]["seed"], "e6a5096ae30c29f6")

    def test_version_information_corruption_is_not_applicable_below_version_seven(self):
        case = next(
            row
            for row in self.plan["mutations"]
            if row["id"] == "v01-H-m0-byte--invalid_version"
        )
        self.assertEqual(case["mutation"]["status"], "not_applicable")
        self.assertEqual(case["mutation"]["expected_outcome"], "reject")


if __name__ == "__main__":
    unittest.main()

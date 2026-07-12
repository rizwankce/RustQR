import importlib.util
import sys
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "qr_spec_mapping.py"
SPEC = importlib.util.spec_from_file_location("qr_spec_mapping", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class QrSpecMappingTests(unittest.TestCase):
    def test_alignment_centers_match_spec_examples(self):
        self.assertEqual(MODULE.alignment_pattern_positions(2), [6, 18])
        self.assertEqual(MODULE.alignment_pattern_positions(7), [6, 22, 38])
        self.assertEqual(MODULE.alignment_pattern_positions(32), [6, 34, 60, 86, 112, 138])

    def test_data_traversal_has_spec_codeword_and_remainder_counts(self):
        expected = {1: (26, 0), 2: (44, 7), 7: (196, 0)}
        for version, (codewords, remainder) in expected.items():
            mapping = MODULE.codeword_to_modules(version)
            self.assertEqual(len(mapping), codewords)
            self.assertEqual(len(MODULE.remainder_modules(version)), remainder)
            flattened = [coordinate for word in mapping for coordinate in word]
            self.assertEqual(len(flattened), len(set(flattened)))

    def test_first_codeword_follows_bottom_right_zigzag(self):
        self.assertEqual(
            MODULE.codeword_to_modules(1)[0],
            ((20, 20), (19, 20), (20, 19), (19, 19),
             (20, 18), (19, 18), (20, 17), (19, 17)),
        )

    def test_rs_map_handles_mixed_short_and_long_blocks(self):
        mapping = MODULE.raw_codeword_block_map(134, 4, 18)
        self.assertEqual(mapping[0], MODULE.BlockCodeword(0, 0, False))
        self.assertEqual(mapping[59], MODULE.BlockCodeword(3, 14, False))
        self.assertEqual(mapping[60], MODULE.BlockCodeword(2, 15, False))
        self.assertEqual(mapping[61], MODULE.BlockCodeword(3, 15, False))
        self.assertEqual(mapping[62], MODULE.BlockCodeword(0, 15, True))
        self.assertEqual(mapping[-1], MODULE.BlockCodeword(3, 33, True))
        for block in range(4):
            expected = 33 if block < 2 else 34
            self.assertEqual(sum(word.block == block for word in mapping), expected)

    def test_invalid_layout_is_rejected(self):
        with self.assertRaises(ValueError):
            MODULE.raw_codeword_block_map(10, 2, 5)


if __name__ == "__main__":
    unittest.main()

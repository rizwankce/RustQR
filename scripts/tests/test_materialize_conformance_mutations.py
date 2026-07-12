import importlib.util
import sys
import unittest
from pathlib import Path


SCRIPTS = Path(__file__).parents[1]
sys.path.insert(0, str(SCRIPTS))
SCRIPT = SCRIPTS / "materialize_conformance_mutations.py"
SPEC = importlib.util.spec_from_file_location("materialize_conformance_mutations", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class MaterializeMutationTests(unittest.TestCase):
    def test_all_mask_formulas_round_trip_raw_bits(self):
        coordinates = ((10, 10), (11, 10), (10, 11), (11, 11))
        bits = (True, False, True, True)
        for mask in range(8):
            modules = [[False] * 21 for _ in range(21)]
            MODULE.set_raw_bits(modules, coordinates, bits, mask)
            self.assertEqual(MODULE.raw_bits(modules, coordinates, mask), bits)

    def test_selection_is_deterministic_and_per_block(self):
        from qr_spec_mapping import raw_codeword_block_map
        mapping = raw_codeword_block_map(134, 4, 18)
        first = MODULE.selected_by_block(mapping, 9, "seed")
        self.assertEqual(first, MODULE.selected_by_block(mapping, 9, "seed"))
        self.assertEqual(set(first), {0, 1, 2, 3})
        for block, indices in first.items():
            self.assertEqual(len(indices), 9)
            self.assertTrue(all(mapping[index].block == block for index in indices))

    def test_cross_block_swap_search_proves_errors_in_both_blocks(self):
        from qr_spec_mapping import raw_codeword_block_map
        mapping = raw_codeword_block_map(134, 4, 18)
        words = [((index, 0),) for index in range(len(mapping))]
        modules = [[False] * len(mapping)]
        # Every raw codeword is deliberately distinct, so a pair must satisfy
        # the requested correction-limit-plus-one boundary.
        for index in range(len(mapping)):
            modules[0][index] = bool(index % 2)
        first = MODULE.unequal_cross_block_swaps(mapping, words, modules, 1, 10, "seed")
        self.assertEqual(first, MODULE.unequal_cross_block_swaps(mapping, words, modules, 1, 10, "seed"))
        self.assertIsNotNone(first)
        left, right, swaps = first
        self.assertEqual(len(swaps), 10)
        for left_index, right_index in swaps:
            self.assertEqual(mapping[left_index].block, left)
            self.assertEqual(mapping[right_index].block, right)
            self.assertNotEqual(
                MODULE.raw_bits(modules, words[left_index], 1),
                MODULE.raw_bits(modules, words[right_index], 1),
            )


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""QR Model 2 data-module and Reed-Solomon block mappings."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class BlockCodeword:
    """Location of an interleaved raw codeword in its RS block."""

    block: int
    index: int
    is_ecc: bool


def alignment_pattern_positions(version: int) -> list[int]:
    """Return Model 2 alignment-pattern center coordinates."""
    if not 1 <= version <= 40:
        raise ValueError("version must be in 1..=40")
    if version == 1:
        return []
    count = version // 7 + 2
    size = version * 4 + 17
    step = 26 if version == 32 else ((version * 4 + count * 2 + 1) // (count * 2 - 2)) * 2
    result = [6]
    result.extend(size - 7 - step * index for index in reversed(range(count - 1)))
    return result


def function_modules(version: int) -> set[tuple[int, int]]:
    """Return every function-module coordinate for a Model 2 symbol."""
    if not 1 <= version <= 40:
        raise ValueError("version must be in 1..=40")
    size = version * 4 + 17
    result: set[tuple[int, int]] = set()

    def rectangle(left: int, top: int, width: int, height: int) -> None:
        for y in range(max(0, top), min(size, top + height)):
            for x in range(max(0, left), min(size, left + width)):
                result.add((x, y))

    rectangle(0, 0, 9, 9)
    rectangle(size - 8, 0, 8, 9)
    rectangle(0, size - 8, 9, 8)
    for coordinate in range(size):
        result.add((6, coordinate))
        result.add((coordinate, 6))
    centers = alignment_pattern_positions(version)
    for center_y in centers:
        for center_x in centers:
            if ((center_x <= 8 and center_y <= 8)
                    or (center_x >= size - 9 and center_y <= 8)
                    or (center_x <= 8 and center_y >= size - 9)):
                continue
            rectangle(center_x - 2, center_y - 2, 5, 5)
    for index in range(9):
        if index != 6:
            result.add((8, index))
            result.add((index, 8))
    for index in range(8):
        result.add((size - 1 - index, 8))
        result.add((8, size - 1 - index))
    result.add((8, size - 8))
    if version >= 7:
        rectangle(size - 11, 0, 3, 6)
        rectangle(0, size - 11, 6, 3)
    return result


def data_module_traversal(version: int) -> list[tuple[int, int]]:
    """Return data modules in raw bitstream order (MSB first)."""
    size = version * 4 + 17
    reserved = function_modules(version)
    result: list[tuple[int, int]] = []
    right = size - 1
    upward = True
    while right >= 1:
        if right == 6:
            right -= 1
        rows = range(size - 1, -1, -1) if upward else range(size)
        for y in rows:
            for x in (right, right - 1):
                if (x, y) not in reserved:
                    result.append((x, y))
        upward = not upward
        right -= 2
    return result


def codeword_to_modules(version: int) -> list[tuple[tuple[int, int], ...]]:
    """Map each complete raw codeword to its eight module coordinates."""
    traversal = data_module_traversal(version)
    return [tuple(traversal[index:index + 8]) for index in range(0, len(traversal) - 7, 8)]


def remainder_modules(version: int) -> tuple[tuple[int, int], ...]:
    traversal = data_module_traversal(version)
    return tuple(traversal[len(traversal) // 8 * 8:])


def raw_codeword_block_map(
    total_codewords: int, num_blocks: int, ecc_per_block: int
) -> list[BlockCodeword]:
    """Map interleaved raw codewords to RS block and within-block indexes."""
    if total_codewords <= 0 or num_blocks <= 0 or ecc_per_block <= 0:
        raise ValueError("codeword and block counts must be positive")
    data_total = total_codewords - num_blocks * ecc_per_block
    if data_total <= 0:
        raise ValueError("RS layout leaves no data codewords")
    short_length, long_blocks = divmod(data_total, num_blocks)
    short_blocks = num_blocks - long_blocks
    result: list[BlockCodeword] = []
    for index in range(short_length + (1 if long_blocks else 0)):
        for block in range(num_blocks):
            data_length = short_length if block < short_blocks else short_length + 1
            if index < data_length:
                result.append(BlockCodeword(block, index, False))
    for ecc_index in range(ecc_per_block):
        for block in range(num_blocks):
            data_length = short_length if block < short_blocks else short_length + 1
            result.append(BlockCodeword(block, data_length + ecc_index, True))
    if len(result) != total_codewords:
        raise AssertionError("internal RS layout length mismatch")
    return result

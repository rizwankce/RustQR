#!/usr/bin/env python3
"""Materialize deterministic QR structural-invalid matrix fixtures.

Coordinates are derived from the repository's explicit Model 2 placement map.
Padding corruption is applied before regenerating Reed-Solomon parity, so its
rejection cannot be explained by an ECC failure.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

from PIL import Image

from qr_spec_mapping import codeword_to_modules, remainder_modules

SCHEMA = "rustqr.structural-invalid-fixtures.v1"


def read_text_matrix(path: Path) -> list[list[int]]:
    rows = [line.strip() for line in path.read_text().splitlines() if line.strip()]
    if not rows or any(len(row) != len(rows) for row in rows):
        raise ValueError(f"{path}: matrix must be non-empty and square")
    if any(set(row) - {"0", "1"} for row in rows):
        raise ValueError(f"{path}: matrix must contain only 0 and 1")
    return [[int(value) for value in row] for row in rows]


def read_png_matrix(path: Path, dimension: int, quiet: int, scale: int) -> list[list[int]]:
    image = Image.open(path).convert("L")
    expected = (dimension + 2 * quiet) * scale
    if image.size != (expected, expected):
        raise ValueError(f"{path}: expected {expected}x{expected}, got {image.size}")
    return [
        [int(image.getpixel(((quiet + x) * scale + scale // 2, (quiet + y) * scale + scale // 2)) < 128) for x in range(dimension)]
        for y in range(dimension)
    ]


def format_coordinates(size: int) -> tuple[list[tuple[int, int]], list[tuple[int, int]]]:
    top_left = [(8, row) for row in range(6)] + [(8, 7), (8, 8), (7, 8)]
    top_left += [(col, 8) for col in reversed(range(6))]
    other = [(size - 1 - offset, 8) for offset in range(8)]
    other += [(8, row) for row in range(size - 7, size)]
    return top_left, other


def version_coordinates(size: int) -> tuple[list[tuple[int, int]], list[tuple[int, int]]]:
    if size < 45:
        raise ValueError("version information exists only for Model 2 version 7+")
    top_right = [(col, row) for row in range(6) for col in range(size - 11, size - 8)]
    bottom_left = [(col, row) for col in range(6) for row in range(size - 11, size - 8)]
    return top_right, bottom_left


def format_codewords() -> list[int]:
    words = []
    for data in range(32):
        remainder = data
        for _ in range(10):
            remainder = (remainder << 1) ^ (((remainder >> 9) & 1) * 0x537)
        words.append(((data << 10) | remainder) ^ 0x5412)
    return words


def maximally_invalid_format_word() -> tuple[int, int]:
    candidates = format_codewords()
    word, distance = max(
        ((word, min((word ^ valid).bit_count() for valid in candidates)) for word in range(1 << 15)),
        key=lambda item: (item[1], -item[0]),
    )
    return word, distance


def write_word(matrix: list[list[int]], coordinates: list[tuple[int, int]], word: int) -> None:
    width = len(coordinates)
    for index, (x, y) in enumerate(coordinates):
        matrix[y][x] = (word >> (width - 1 - index)) & 1


def write_matrix(path: Path, matrix: list[list[int]]) -> str:
    data = "".join("".join(str(value) for value in row) + "\n" for row in matrix).encode()
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    return hashlib.sha256(data).hexdigest()


def mask_applies(pattern: int, x: int, y: int) -> bool:
    predicates = (
        (x + y) % 2 == 0,
        y % 2 == 0,
        x % 3 == 0,
        (x + y) % 3 == 0,
        (y // 2 + x // 3) % 2 == 0,
        x * y % 2 + x * y % 3 == 0,
        (x * y % 2 + x * y % 3) % 2 == 0,
        ((x + y) % 2 + x * y % 3) % 2 == 0,
    )
    return predicates[pattern]


def write_raw_codewords(
    matrix: list[list[int]], version: int, mask: int, codewords: list[int]
) -> None:
    coordinates = codeword_to_modules(version)
    if len(coordinates) != len(codewords):
        raise ValueError(f"version {version}: codeword count mismatch")
    for value, modules in zip(codewords, coordinates):
        for bit_index, (x, y) in enumerate(modules):
            raw_bit = (value >> (7 - bit_index)) & 1
            matrix[y][x] = raw_bit ^ mask_applies(mask, x, y)


def invalid_padding_codewords(payload: bytes) -> tuple[list[int], int, int, int]:
    """Return v1-M/mask-0 bytes with valid ECC but a non-standard first pad."""
    import qrcode
    from qrcode import base, constants, util

    version = 1
    blocks = base.rs_blocks(version, constants.ERROR_CORRECT_M)
    bit_limit = sum(block.data_count * 8 for block in blocks)
    data = util.QRData(payload, mode=util.MODE_8BIT_BYTE, check_data=False)
    buffer = util.BitBuffer()
    buffer.put(data.mode, 4)
    buffer.put(len(data), util.length_in_bits(data.mode, version))
    data.write(buffer)
    for _ in range(min(bit_limit - len(buffer), 4)):
        buffer.put_bit(False)
    while len(buffer) % 8:
        buffer.put_bit(False)
    pad_index = len(buffer) // 8
    bytes_to_fill = (bit_limit - len(buffer)) // 8
    for index in range(bytes_to_fill):
        buffer.put(util.PAD0 if index % 2 == 0 else util.PAD1, 8)
    old_value = buffer.buffer[pad_index]
    new_value = old_value ^ 1
    buffer.buffer[pad_index] = new_value
    return util.create_bytes(buffer, blocks), pad_index, old_value, new_value


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-dir", type=Path, default=Path("tests/conformance/fixtures"))
    parser.add_argument("--manifest", type=Path, default=Path("tests/conformance/structural-invalid.json"))
    args = parser.parse_args()

    format_matrix = read_text_matrix(args.output_dir / "golden-v1-m.matrix")
    format_word, format_distance = maximally_invalid_format_word()
    for coordinates in format_coordinates(len(format_matrix)):
        write_word(format_matrix, coordinates, format_word)
    format_path = args.output_dir / "invalid-format-v1.matrix"

    version_matrix = read_png_matrix(Path("conformance/matrices/v07-L-m0-byte.png"), 45, 4, 4)
    for coordinates in version_coordinates(len(version_matrix)):
        write_word(version_matrix, coordinates, 0)
    version_path = args.output_dir / "invalid-version-v7.matrix"

    remainder_matrix = read_png_matrix(Path("conformance/matrices/v02-M-m0-byte.png"), 25, 4, 4)
    remainder_coordinates = remainder_modules(2)
    if len(remainder_coordinates) != 7:
        raise AssertionError("Model 2 version 2 must have seven remainder modules")
    remainder_coordinate = remainder_coordinates[0]
    remainder_x, remainder_y = remainder_coordinate
    before = remainder_matrix[remainder_y][remainder_x]
    remainder_matrix[remainder_y][remainder_x] ^= 1
    remainder_path = args.output_dir / "invalid-remainder-v2.matrix"

    padding_matrix = read_png_matrix(Path("conformance/matrices/v01-M-m0-byte.png"), 21, 4, 4)
    payload = bytes.fromhex("323934326161")
    padding_codewords, padding_index, padding_before, padding_after = invalid_padding_codewords(payload)
    write_raw_codewords(padding_matrix, 1, 0, padding_codewords)
    padding_path = args.output_dir / "invalid-padding-v1.matrix"
    padding_coordinates = codeword_to_modules(1)[padding_index]

    cases = [
        {
            "id": "invalid-format-v1",
            "kind": "invalid_format",
            "parent": "golden-v1-m-numeric",
            "matrix": format_path.name,
            "sha256": write_matrix(format_path, format_matrix),
            "coordinate_source": "ISO format information copies",
            "mutation": {"word": f"{format_word:015b}", "minimum_valid_bch_distance": format_distance},
        },
        {
            "id": "invalid-version-v7",
            "kind": "invalid_version",
            "parent": "v07-L-m0-byte",
            "matrix": version_path.name,
            "sha256": write_matrix(version_path, version_matrix),
            "coordinate_source": "ISO version information copies",
            "mutation": {"word": "0" * 18, "decoded_version": None},
        },
        {
            "id": "invalid-remainder-v2",
            "kind": "invalid_remainder",
            "parent": "v02-M-m0-byte",
            "matrix": remainder_path.name,
            "sha256": write_matrix(remainder_path, remainder_matrix),
            "coordinate_source": "scripts/qr_spec_mapping.py remainder_modules(2)",
            "mutation": {
                "coordinate": list(remainder_coordinate),
                "masked_before": before,
                "masked_after": remainder_matrix[remainder_y][remainder_x],
                "unmasked_after": 1,
                "remainder_module_count": len(remainder_coordinates),
            },
        },
        {
            "id": "invalid-padding-v1",
            "kind": "invalid_padding",
            "parent": "v01-M-m0-byte",
            "matrix": padding_path.name,
            "sha256": write_matrix(padding_path, padding_matrix),
            "coordinate_source": "scripts/qr_spec_mapping.py codeword_to_modules(1)",
            "mutation": {
                "data_codeword_index": padding_index,
                "module_coordinates_msb_first": [list(item) for item in padding_coordinates],
                "expected_pad_codeword": f"{padding_before:02x}",
                "replacement_codeword": f"{padding_after:02x}",
                "reed_solomon_parity": "regenerated with python-qrcode==8.2",
            },
        },
    ]
    document = {
        "schema_version": SCHEMA,
        "generator": "scripts/materialize_structural_invalid_fixtures.py",
        "cases": cases,
        "provenance": {
            "placement_map": "scripts/qr_spec_mapping.py",
            "padding_backend": "python-qrcode==8.2",
        },
    }
    args.manifest.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()

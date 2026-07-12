#!/usr/bin/env python3
"""Materialize mapping-backed WP-006 matrix mutations."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path

from PIL import Image

from qr_spec_mapping import codeword_to_modules, raw_codeword_block_map

SCHEMA = "rustqr.conformance-mutations.v1"
EC_INDEX = {"L": 0, "M": 1, "Q": 2, "H": 3}


def rust_table(source: str, name: str) -> list[list[int]]:
    start = source.index(f"const {name}")
    values_start = source.index("= [", start) + 2
    end = source.index("];", values_start) + 1
    values = [int(value) for value in re.findall(r"-?\d+", source[values_start:end])]
    if len(values) != 4 * 41:
        raise ValueError(f"unexpected {name} shape")
    return [values[index:index + 41] for index in range(0, len(values), 41)]


def stable_order(indices: list[int], seed: str) -> list[int]:
    return sorted(indices, key=lambda index: hashlib.sha256(f"{seed}:{index}".encode()).digest())


def mask_applies(pattern: int, x: int, y: int) -> bool:
    formulas = (
        (x + y) % 2 == 0,
        y % 2 == 0,
        x % 3 == 0,
        (x + y) % 3 == 0,
        (y // 2 + x // 3) % 2 == 0,
        x * y % 2 + x * y % 3 == 0,
        (x * y % 2 + x * y % 3) % 2 == 0,
        ((x + y) % 2 + x * y % 3) % 2 == 0,
    )
    if not 0 <= pattern < len(formulas):
        raise ValueError("mask must be in 0..=7")
    return formulas[pattern]


def load_modules(case: dict, root: Path) -> list[list[bool]]:
    metadata = case["matrix"]
    image = Image.open(root / metadata["path"]).convert("L")
    dimension = metadata["symbol_dimension"]
    quiet = metadata["quiet_zone_modules"]
    scale = metadata["pixels_per_module"]
    return [[
        image.getpixel(((quiet + x) * scale + scale // 2, (quiet + y) * scale + scale // 2)) < 128
        for x in range(dimension)
    ] for y in range(dimension)]


def save_modules(modules: list[list[bool]], path: Path, quiet: int, scale: int) -> str:
    size = len(modules)
    image = Image.new("L", ((size + quiet * 2) * scale, (size + quiet * 2) * scale), 255)
    pixels = image.load()
    for y, row in enumerate(modules):
        for x, dark in enumerate(row):
            if dark:
                for py in range((quiet + y) * scale, (quiet + y + 1) * scale):
                    for px in range((quiet + x) * scale, (quiet + x + 1) * scale):
                        pixels[px, py] = 0
    path.parent.mkdir(parents=True, exist_ok=True)
    image.save(path)
    return hashlib.sha256(path.read_bytes()).hexdigest()


def raw_bits(modules: list[list[bool]], coordinates, mask: int) -> tuple[bool, ...]:
    return tuple(modules[y][x] ^ mask_applies(mask, x, y) for x, y in coordinates)


def set_raw_bits(modules: list[list[bool]], coordinates, bits, mask: int) -> list[list[int]]:
    changed = []
    for (x, y), bit in zip(coordinates, bits):
        value = bit ^ mask_applies(mask, x, y)
        if modules[y][x] != value:
            modules[y][x] = value
            changed.append([x, y])
    return changed


def selected_by_block(mapping, count: int, seed: str) -> dict[int, list[int]]:
    blocks = sorted({entry.block for entry in mapping})
    result = {}
    for block in blocks:
        candidates = [index for index, entry in enumerate(mapping) if entry.block == block]
        result[block] = stable_order(candidates, f"{seed}:block-{block}")[:count]
    return result


def unequal_cross_block_swaps(mapping, words, modules, mask: int, count: int, seed: str):
    """Find deterministic swaps which exceed the correction limit in two blocks.

    A swap is useful only when the two raw codewords differ: then both involved
    RS blocks receive one definite symbol error.  Search every block pair rather
    than relying on a small random prefix, which can contain mostly identical
    padding bytes in otherwise valid symbols.
    """
    by_block = {
        block: stable_order(
            [index for index, entry in enumerate(mapping) if entry.block == block],
            f"{seed}:block-{block}",
        )
        for block in sorted({entry.block for entry in mapping})
    }
    values = [raw_bits(modules, coordinates, mask) for coordinates in words]
    for left_block in sorted(by_block):
        for right_block in sorted(block for block in by_block if block > left_block):
            left = by_block[left_block]
            right = by_block[right_block]
            # Standard augmenting-path matching prevents a greedy choice from
            # consuming the only unequal partner for a later codeword.
            matched_left: dict[int, int] = {}
            matched_right: dict[int, int] = {}

            def assign(left_index: int, seen: set[int]) -> bool:
                for right_index in right:
                    if right_index in seen or values[left_index] == values[right_index]:
                        continue
                    seen.add(right_index)
                    incumbent = matched_right.get(right_index)
                    if incumbent is None or assign(incumbent, seen):
                        matched_left[left_index] = right_index
                        matched_right[right_index] = left_index
                        return True
                return False

            for left_index in left:
                assign(left_index, set())
            swaps = [(index, matched_left[index]) for index in left if index in matched_left]
            if len(swaps) >= count:
                return left_block, right_block, swaps[:count]
    return None


def materialize(manifest_path: Path, tables_path: Path, output_path: Path) -> dict:
    manifest_bytes = manifest_path.read_bytes()
    manifest = json.loads(manifest_bytes)
    source = tables_path.read_text()
    ecc_table = rust_table(source, "ECC_CODEWORDS_PER_BLOCK")
    block_table = rust_table(source, "NUM_ERROR_CORRECTION_BLOCKS")
    root = manifest_path.parent
    output_root = output_path.parent / "mutations"
    rows = []
    for case in manifest["cases"]:
        if case["matrix"]["status"] != "generated":
            continue
        version = case["version"]
        ec_index = EC_INDEX[case["ec_level"]]
        ecc = ecc_table[ec_index][version]
        blocks = block_table[ec_index][version]
        words = codeword_to_modules(version)
        block_map = raw_codeword_block_map(len(words), blocks, ecc)
        base = load_modules(case, root)
        quiet = case["matrix"]["quiet_zone_modules"]
        scale = case["matrix"]["pixels_per_module"]

        error_seed = hashlib.sha256(f"{case['seed']}:{case['id']}:errors".encode()).hexdigest()[:16]
        selected = selected_by_block(block_map, ecc // 2, error_seed)
        mutated = [row[:] for row in base]
        changed = []
        for indices in selected.values():
            for raw_index in indices:
                x, y = words[raw_index][0]
                mutated[y][x] = not mutated[y][x]
                changed.append([x, y])
        relative = Path("mutations") / f"{case['id']}--correctable-errors.png"
        sha = save_modules(mutated, output_path.parent / relative, quiet, scale)
        rows.append({
            "id": f"{case['id']}--correctable_errors", "parent_id": case["id"],
            "kind": "correctable_errors", "status": "materialized", "expected_outcome": "decode_success",
            "seed": error_seed, "errors_per_block": ecc // 2, "selected_raw_codewords": selected,
            "changed_modules": changed, "expected_raw_payload_hex": case["expected"]["raw_payload_hex"],
            "matrix": {"path": relative.as_posix(), "sha256": sha, **{key: case["matrix"][key] for key in ("symbol_dimension", "quiet_zone_modules", "pixels_per_module")}},
        })

        erasure_seed = hashlib.sha256(f"{case['seed']}:{case['id']}:erasures".encode()).hexdigest()[:16]
        erasures = selected_by_block(block_map, ecc, erasure_seed)
        mutated = [row[:] for row in base]
        changed = []
        low_confidence = []
        for indices in erasures.values():
            for raw_index in indices:
                x, y = words[raw_index][0]
                mutated[y][x] = not mutated[y][x]
                changed.append([x, y])
                low_confidence.extend([list(coordinate) for coordinate in words[raw_index]])
        relative = Path("mutations") / f"{case['id']}--correctable-erasures.png"
        sha = save_modules(mutated, output_path.parent / relative, quiet, scale)
        sidecar_relative = Path("mutations") / f"{case['id']}--correctable-erasures.confidence.json"
        sidecar = {"schema_version": "rustqr.module-confidence.v1", "default_confidence": 255,
                   "low_confidence": 0, "module_coordinates": low_confidence}
        sidecar_path = output_path.parent / sidecar_relative
        sidecar_path.write_text(json.dumps(sidecar, indent=2, sort_keys=True) + "\n")
        rows.append({
            "id": f"{case['id']}--correctable_erasures", "parent_id": case["id"],
            "kind": "correctable_erasures", "status": "materialized",
            "reason": "executable through decode_matrix_with_erasures using the recorded sidecar coordinates",
            "expected_outcome": "decode_success", "seed": erasure_seed, "erasures_per_block": ecc,
            "selected_raw_codewords": erasures, "changed_modules": changed,
            "expected_raw_payload_hex": case["expected"]["raw_payload_hex"],
            "confidence_sidecar": {"path": sidecar_relative.as_posix(), "sha256": hashlib.sha256(sidecar_path.read_bytes()).hexdigest()},
            "matrix": {"path": relative.as_posix(), "sha256": sha, **{key: case["matrix"][key] for key in ("symbol_dimension", "quiet_zone_modules", "pixels_per_module")}},
        })

        if blocks < 2:
            rows.append({"id": f"{case['id']}--invalid_block_layout", "parent_id": case["id"],
                         "kind": "invalid_block_layout", "status": "not_applicable",
                         "reason": "symbol has one RS block"})
            continue
        invalid_seed = hashlib.sha256(f"{case['seed']}:{case['id']}:block-layout".encode()).hexdigest()[:16]
        required_swaps = ecc // 2 + 1
        swap_plan = unequal_cross_block_swaps(
            block_map, words, base, case["mask"], required_swaps, invalid_seed
        )
        if swap_plan is None:
            rows.append({"id": f"{case['id']}--invalid_block_layout", "parent_id": case["id"],
                         "kind": "invalid_block_layout", "status": "not_applicable",
                         "reason": "no pair of RS blocks has enough unequal codewords for a provable swap"})
            continue
        left_block, right_block, swaps = swap_plan
        mutated = [row[:] for row in base]
        changed = []
        for left_index, right_index in swaps:
            left_bits = raw_bits(base, words[left_index], case["mask"])
            right_bits = raw_bits(base, words[right_index], case["mask"])
            changed.extend(set_raw_bits(mutated, words[left_index], right_bits, case["mask"]))
            changed.extend(set_raw_bits(mutated, words[right_index], left_bits, case["mask"]))
        relative = Path("mutations") / f"{case['id']}--invalid-block-layout.png"
        sha = save_modules(mutated, output_path.parent / relative, quiet, scale)
        rows.append({
            "id": f"{case['id']}--invalid_block_layout", "parent_id": case["id"],
            "kind": "invalid_block_layout", "status": "materialized", "expected_outcome": "reject",
            "seed": invalid_seed, "swapped_raw_codewords": swaps, "changed_modules": changed,
            "affected_blocks": [left_block, right_block], "errors_per_affected_block": len(swaps),
            "correction_limit_per_block": ecc // 2,
            "proof": "each unequal cross-block swap creates one definite symbol error in both affected RS blocks",
            "matrix": {"path": relative.as_posix(), "sha256": sha, **{key: case["matrix"][key] for key in ("symbol_dimension", "quiet_zone_modules", "pixels_per_module")}},
        })
    result = {"schema_version": SCHEMA, "source_manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(),
              "tables_sha256": hashlib.sha256(tables_path.read_bytes()).hexdigest(), "mutations": rows}
    output_path.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    return result


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=Path("conformance/manifest.json"))
    parser.add_argument("--tables", type=Path, default=Path("src/decoder/tables.rs"))
    parser.add_argument("--output", type=Path, default=Path("conformance/mutations.json"))
    args = parser.parse_args()
    materialize(args.manifest, args.tables, args.output)


if __name__ == "__main__":
    main()

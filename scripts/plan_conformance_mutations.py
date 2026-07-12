#!/usr/bin/env python3
"""Create deterministic, non-fabricated corruption plans for WP-006 fixtures."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path

SCHEMA = "rustqr.conformance-mutation-plan.v1"
MANIFEST_SCHEMA = "rustqr.conformance-manifest.v1"
EC_INDEX = {"L": 0, "M": 1, "Q": 2, "H": 3}


def rust_table(source: str, name: str) -> list[list[int]]:
    start = source.index(f"const {name}")
    values_start = source.index("= [", start) + 2
    end = source.index("];", values_start) + 1
    values = [int(value) for value in re.findall(r"-?\d+", source[values_start:end])]
    if len(values) != 4 * 41:
        raise ValueError(f"unexpected {name} shape: {len(values)} values")
    return [values[index : index + 41] for index in range(0, len(values), 41)]


def stable_seed(case_id: str, kind: str, manifest_seed: str) -> str:
    return hashlib.sha256(f"{manifest_seed}:{case_id}:{kind}".encode()).hexdigest()[:16]


def plan_for_case(case: dict, ecc_table: list[list[int]], manifest_seed: str) -> list[dict]:
    version = int(case["version"])
    ecc = ecc_table[EC_INDEX[case["ec_level"]]][version]
    common = {"parent_id": case["id"], "expected_raw_payload_hex": case["expected"]["raw_payload_hex"]}
    definitions = [
        (
            "correctable_errors",
            "decode_success",
            ["codeword_to_module_map", "rs_block_map"],
            {"errors_per_block": ecc // 2, "limit_basis": "floor(ecc_codewords_per_block/2)"},
        ),
        (
            "correctable_erasures",
            "decode_success",
            ["codeword_to_module_map", "rs_block_map", "erasure_confidence_sidecar"],
            {"erasures_per_block": ecc, "limit_basis": "ecc_codewords_per_block"},
        ),
        ("invalid_format", "reject", ["format_module_map"], {}),
        ("invalid_version", "reject", ["version_module_map"], {}),
        ("invalid_remainder", "reject", ["remainder_module_map"], {}),
        ("invalid_padding", "reject", ["codeword_to_module_map", "padding_codeword_map"], {}),
        ("invalid_block_layout", "reject", ["codeword_to_module_map", "rs_block_map"], {}),
    ]
    plans = []
    for kind, outcome, requirements, parameters in definitions:
        applicable = kind != "invalid_version" or version >= 7
        plans.append(
            {
                **common,
                "id": f"{case['id']}--{kind}",
                "mutation": {
                    "kind": kind,
                    "seed": stable_seed(case["id"], kind, manifest_seed),
                    "status": "planned" if applicable else "not_applicable",
                    "expected_outcome": outcome,
                    "backend_requirements": requirements,
                    "parameters": {"ecc_codewords_per_block": ecc, **parameters},
                },
            }
        )
    return plans


def generate(manifest_path: Path, tables_path: Path) -> dict:
    manifest_bytes = manifest_path.read_bytes()
    manifest = json.loads(manifest_bytes)
    if manifest.get("schema_version") != MANIFEST_SCHEMA:
        raise ValueError(f"expected {MANIFEST_SCHEMA} manifest")
    tables_bytes = tables_path.read_bytes()
    ecc_table = rust_table(tables_bytes.decode(), "ECC_CODEWORDS_PER_BLOCK")
    manifest_seed = manifest.get("generator", {}).get("seed", "")
    plans = [
        plan
        for case in manifest.get("cases", [])
        for plan in plan_for_case(case, ecc_table, manifest_seed)
    ]
    return {
        "schema_version": SCHEMA,
        "manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(),
        "tables_sha256": hashlib.sha256(tables_bytes).hexdigest(),
        "semantics": {
            "planned": "not executable and never counted as pass or failure",
            "decode_success": "materialized fixture must reproduce expected raw payload",
            "reject": "materialized fixture must not return an accepted payload",
        },
        "mutations": plans,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=Path("conformance/manifest.json"))
    parser.add_argument("--tables", type=Path, default=Path("src/decoder/tables.rs"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = generate(args.manifest, args.tables)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()

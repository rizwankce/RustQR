#!/usr/bin/env python3
"""Generate deterministic QR Model 2 conformance case manifests.

This generator intentionally describes cases without pretending to encode QR
matrices. Independent encoder adapters can materialize `matrix_path` later;
the payload and expected metadata remain stable across adapters and runs.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

SCHEMA_VERSION = "rustqr.conformance-manifest.v1"
DEFAULT_SEED = "rustqr-model2-conformance-v1"
EC_LEVELS = ("L", "M", "Q", "H")
MASKS = tuple(range(8))
MODES = ("numeric", "alphanumeric", "byte", "kanji", "eci", "gs1_fnc1", "structured_append")
SMOKE_VERSIONS = (1, 2, 7, 20, 40)


def deterministic_payload(seed: str, version: int, ec_level: str, mask: int, mode: str) -> bytes:
    digest = hashlib.sha256(f"{seed}:{version}:{ec_level}:{mask}:{mode}".encode()).digest()
    if mode == "numeric":
        return "".join(str(byte % 10) for byte in digest[:8]).encode("ascii")
    if mode == "alphanumeric":
        alphabet = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:"
        return bytes(alphabet[byte % len(alphabet)] for byte in digest[:6])
    if mode == "kanji":
        return "漢字".encode("shift_jis")
    if mode == "gs1_fnc1":
        return b"0109501101530003\x1d17271231"
    if mode == "structured_append":
        return b"RUSTQR-STRUCTURED-APPEND"
    if mode == "eci":
        # Assignment 26 identifies UTF-8. Keep this fixture valid UTF-8 so
        # both raw-byte and text-only differential decoders can independently
        # verify its payload as well as its ECI header.
        return b"RustQR ECI 26"
    if mode == "byte":
        # Keep foundation byte fixtures valid UTF-8 so both raw-byte and
        # text-only differential decoders can verify the same expected bytes.
        return digest[:3].hex().encode("ascii")
    return digest[:4]


def case_record(seed: str, version: int, ec_level: str, mask: int, mode: str) -> dict:
    payload = deterministic_payload(seed, version, ec_level, mask, mode)
    case_key = f"v{version:02}-{ec_level}-m{mask}-{mode}"
    expected = {
        "raw_payload_hex": payload.hex(),
        "version": version,
        "ec_level": ec_level,
        "mask": mask,
        "mode": mode,
    }
    # These representative symbols contain QR headers in addition to their
    # payload segment. Keep the expected header values in the manifest so an
    # end-to-end matrix test can assert the public decoder metadata too.
    if mode == "eci":
        expected["metadata"] = {"eci_assignment": 26}
    elif mode == "gs1_fnc1":
        expected["metadata"] = {"fnc1": {"position": "first"}}
    elif mode == "structured_append":
        expected["metadata"] = {
            "structured_append": {"index": 2, "total_symbols": 4, "parity": 0xA7}
        }
    return {
        "id": case_key,
        "seed": seed,
        "model": 2,
        "version": version,
        "ec_level": ec_level,
        "mask": mask,
        "mode": mode,
        "capacity_boundary": "representative",
        "payload_hex": payload.hex(),
        "expected": expected,
        "matrix": {
            "path": None,
            "sha256": None,
            "generator_backend": None,
            "status": "planned",
        },
        "differential": {"decoders": [], "status": "pending"},
    }


def generate_manifest(seed: str = DEFAULT_SEED, full: bool = False) -> dict:
    versions = range(1, 41) if full else SMOKE_VERSIONS
    if full:
        tuples = (
            (version, ec_level, mask, mode)
            for version in versions
            for ec_level in EC_LEVELS
            for mask in MASKS
            for mode in MODES
        )
    else:
        # Cover each axis without checking in the full Cartesian product.
        header_versions = {"gs1_fnc1": 2, "structured_append": 2}
        tuples = iter(
            sorted(
                {(v, ec, 0, "byte") for v in versions for ec in EC_LEVELS}
                | {(1, "L", mask, "byte") for mask in MASKS}
                | {(header_versions.get(mode, 1), "M", 3, mode) for mode in MODES}
            )
        )
    cases = [case_record(seed, *case) for case in tuples]
    return {
        "schema_version": SCHEMA_VERSION,
        "generator": {
            "script": "scripts/generate_conformance_manifest.py",
            "seed": seed,
            "profile": "full" if full else "foundation",
        },
        "coverage": {
            "versions": list(versions),
            "ec_levels": list(EC_LEVELS),
            "masks": list(MASKS),
            "modes": list(MODES),
            "planned_capacity_boundaries": ["empty", "representative", "maximum", "overflow"],
            "planned_matrix_variants": [
                "valid",
                "correctable_errors",
                "correctable_erasures",
                "invalid_format",
                "invalid_version",
                "invalid_remainder",
                "invalid_padding",
                "invalid_block_layout",
            ],
        },
        "cases": cases,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--seed", default=DEFAULT_SEED)
    parser.add_argument("--full", action="store_true", help="emit all versions, EC levels, masks, and mode scaffolds")
    args = parser.parse_args()
    manifest = generate_manifest(args.seed, args.full)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()

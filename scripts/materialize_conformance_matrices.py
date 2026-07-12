#!/usr/bin/env python3
"""Materialize supported manifest cases with pinned python-qrcode 8.2."""

from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import json
from pathlib import Path

BACKEND = "python-qrcode==8.2"
SUPPORTED_MODES = {"numeric", "alphanumeric", "byte"}


def maximum_payload_length(qrcode, util, version, ec_value, mode_value, unit: bytes) -> int:
    def fits(length: int) -> bool:
        qr = qrcode.QRCode(version=version, error_correction=ec_value, border=0)
        qr.add_data(util.QRData(unit * length, mode=mode_value, check_data=False), optimize=0)
        try:
            qr.make(fit=False)
            return True
        except qrcode.exceptions.DataOverflowError:
            return False

    low, high = 0, 1
    while fits(high):
        low, high = high, high * 2
    while low + 1 < high:
        middle = (low + high) // 2
        if fits(middle):
            low = middle
        else:
            high = middle
    return low


def materialize(manifest_path: Path) -> dict:
    import qrcode
    from qrcode import constants, util

    version = importlib.metadata.version("qrcode")
    if version != "8.2":
        raise RuntimeError(f"expected qrcode 8.2, found {version}")
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if manifest.get("schema_version") != "rustqr.conformance-manifest.v1":
        raise ValueError("unsupported conformance manifest schema")

    ec_values = {
        "L": constants.ERROR_CORRECT_L,
        "M": constants.ERROR_CORRECT_M,
        "Q": constants.ERROR_CORRECT_Q,
        "H": constants.ERROR_CORRECT_H,
    }
    mode_values = {
        "numeric": util.MODE_NUMBER,
        "alphanumeric": util.MODE_ALPHA_NUM,
        "byte": util.MODE_8BIT_BYTE,
    }
    mode_units = {"numeric": b"0", "alphanumeric": b"A", "byte": b"x"}
    matrix_root = manifest_path.parent / "matrices"
    matrix_root.mkdir(parents=True, exist_ok=True)
    for case in manifest["cases"]:
        mode = case["mode"]
        if mode not in SUPPORTED_MODES:
            case["matrix"]["status"] = "unsupported_mode"
            case["matrix"]["generator_backend"] = BACKEND
            continue
        payload = bytes.fromhex(case["payload_hex"])
        qr = qrcode.QRCode(
            version=case["version"],
            error_correction=ec_values[case["ec_level"]],
            mask_pattern=case["mask"],
            box_size=4,
            border=4,
        )
        qr.add_data(util.QRData(payload, mode=mode_values[mode], check_data=False), optimize=0)
        qr.make(fit=False)
        relative = Path("matrices") / f"{case['id']}.png"
        output = manifest_path.parent / relative
        qr.make_image(fill_color="black", back_color="white").save(output)
        file_bytes = output.read_bytes()
        case["matrix"] = {
            "path": relative.as_posix(),
            "sha256": hashlib.sha256(file_bytes).hexdigest(),
            "generator_backend": BACKEND,
            "status": "generated",
            "symbol_dimension": 17 + 4 * case["version"],
            "quiet_zone_modules": 4,
            "pixels_per_module": 4,
        }
    capacity_keys = sorted(
        {(case["version"], case["ec_level"], case["mode"]) for case in manifest["cases"] if case["mode"] in SUPPORTED_MODES}
    )
    manifest["capacity_validation"] = [
        {
            "version": version,
            "ec_level": ec_level,
            "mode": mode,
            "maximum_units": maximum_payload_length(
                qrcode, util, version, ec_values[ec_level], mode_values[mode], mode_units[mode]
            ),
            "maximum_accepted": True,
            "maximum_plus_one_rejected": True,
            "generator_backend": BACKEND,
        }
        for version, ec_level, mode in capacity_keys
    ]
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return manifest


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=Path("conformance/manifest.json"))
    args = parser.parse_args()
    materialize(args.manifest)


if __name__ == "__main__":
    main()

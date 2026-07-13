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
HEADER_MODES = {"kanji", "eci", "gs1_fnc1", "structured_append"}
HEADER_BACKEND = "python-qrcode==8.2+iso-header-adapter-v1"


class Segment:
    """A QR segment whose complete ISO bit representation is written here.

    python-qrcode exposes normal numeric/alphanumeric/byte segments but not
    the header modes needed by these fixtures. This tiny adapter deliberately
    builds those segment bits itself while still using python-qrcode for the
    RS/interleaving and module-placement implementation.
    """

    def __init__(self, mode: int, count: int | None, bits: list[tuple[int, int]]):
        self.mode = mode
        self.count = count
        self.bits = bits

    def write(self, buffer, version: int, util) -> None:
        buffer.put(self.mode, 4)
        if self.count is not None:
            buffer.put(self.count, util.length_in_bits(self.mode, version))
        for value, width in self.bits:
            buffer.put(value, width)


def alphanumeric_bits(data: bytes, util) -> list[tuple[int, int]]:
    values = [util.ALPHA_NUM.find(bytes([byte])) for byte in data]
    if any(value < 0 for value in values):
        raise ValueError("GS1 fixture data must be QR alphanumeric")
    bits = []
    for index in range(0, len(values), 2):
        if index + 1 < len(values):
            bits.append((values[index] * 45 + values[index + 1], 11))
        else:
            bits.append((values[index], 6))
    return bits


def kanji_bits(data: bytes) -> list[tuple[int, int]]:
    if len(data) % 2:
        raise ValueError("Kanji payload must contain complete Shift-JIS pairs")
    bits = []
    for index in range(0, len(data), 2):
        code = (data[index] << 8) | data[index + 1]
        if 0x8140 <= code <= 0x9FFC:
            subtracted = code - 0x8140
        elif 0xE040 <= code <= 0xEBBF:
            subtracted = code - 0xC140
        else:
            raise ValueError(f"not QR Kanji Shift-JIS: {code:04x}")
        bits.append((((subtracted >> 8) * 0xC0) + (subtracted & 0xFF), 13))
    return bits


def header_segments(case: dict, util) -> list[Segment]:
    payload = bytes.fromhex(case["payload_hex"])
    mode = case["mode"]
    if mode == "kanji":
        return [Segment(util.MODE_KANJI, len(payload) // 2, kanji_bits(payload))]
    if mode == "eci":
        # Assignment 26 identifies UTF-8. The fixture payload is its UTF-8
        # byte representation, allowing exact raw-byte verification.
        return [Segment(7, None, [(26, 8)]), Segment(util.MODE_8BIT_BYTE, len(payload), [(byte, 8) for byte in payload])]
    if mode == "gs1_fnc1":
        # In GS1 alphanumeric data, '%' encodes the FNC1 group separator.
        encoded = payload.replace(b"\x1d", b"%")
        return [Segment(5, None, []), Segment(util.MODE_ALPHA_NUM, len(encoded), alphanumeric_bits(encoded, util))]
    if mode == "structured_append":
        return [
            Segment(3, None, [(2, 4), (3, 4), (0xA7, 8)]),
            Segment(util.MODE_ALPHA_NUM, len(payload), alphanumeric_bits(payload, util)),
        ]
    raise ValueError(f"not a header fixture mode: {mode}")


def create_header_data(qrcode, util, version: int, ec_value: int, segments: list[Segment]) -> list[int]:
    """Equivalent to qrcode.util.create_data, with header-mode support."""
    buffer = util.BitBuffer()
    for segment in segments:
        segment.write(buffer, version, util)
    rs_blocks = qrcode.base.rs_blocks(version, ec_value)
    bit_limit = sum(block.data_count * 8 for block in rs_blocks)
    if len(buffer) > bit_limit:
        raise qrcode.exceptions.DataOverflowError(
            f"Code length overflow: {len(buffer)} > {bit_limit}"
        )
    for _ in range(min(bit_limit - len(buffer), 4)):
        buffer.put_bit(False)
    if delimit := len(buffer) % 8:
        for _ in range(8 - delimit):
            buffer.put_bit(False)
    for index in range((bit_limit - len(buffer)) // 8):
        buffer.put(util.PAD0 if index % 2 == 0 else util.PAD1, 8)
    return util.create_bytes(buffer, rs_blocks)


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
    materialize_header_cases = manifest.get("generator", {}).get("profile") == "foundation"

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
        if mode not in SUPPORTED_MODES | HEADER_MODES or (mode in HEADER_MODES and not materialize_header_cases):
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
        if mode in SUPPORTED_MODES:
            qr.add_data(util.QRData(payload, mode=mode_values[mode], check_data=False), optimize=0)
            backend = BACKEND
        else:
            qr.data_cache = create_header_data(qrcode, util, case["version"], ec_values[case["ec_level"]], header_segments(case, util))
            backend = HEADER_BACKEND
        qr.make(fit=False)
        relative = Path("matrices") / f"{case['id']}.png"
        output = manifest_path.parent / relative
        qr.make_image(fill_color="black", back_color="white").save(output)
        file_bytes = output.read_bytes()
        case["matrix"] = {
            "path": relative.as_posix(),
            "sha256": hashlib.sha256(file_bytes).hexdigest(),
            "generator_backend": backend,
            "status": "generated",
            "symbol_dimension": 17 + 4 * case["version"],
            "quiet_zone_modules": 4,
            "pixels_per_module": 4,
        }
    # Capacity-boundary evidence belongs to the compact foundation corpus.  The
    # full-grid gate deliberately expands masks, not capacity probes; repeating
    # the binary searches for every version/EC/mode makes that gate depend on a
    # python-qrcode overflow-path bug (``glog(0)`` for some known-overflow
    # candidates) without adding coverage.  Keep the checked-in representative
    # probes, and let the full profile concentrate on its 3,840 valid symbols.
    capacity_keys = (
        sorted(
            {
                (case["version"], case["ec_level"], case["mode"])
                for case in manifest["cases"]
                if case["mode"] in SUPPORTED_MODES
            }
        )
        if materialize_header_cases
        else []
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

#!/usr/bin/env python3
"""Verify the pinned Wikimedia Commons negative-corpus asset."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CORPUS_ROOT = ROOT / "tests" / "negative_corpus"
MANIFEST_PATH = CORPUS_ROOT / "wikimedia_commons_manifest.json"


def jpeg_dimensions(path: Path) -> tuple[int, int]:
    """Read dimensions from a baseline or progressive JPEG SOF segment."""
    data = path.read_bytes()
    if data[:2] != b"\xff\xd8":
        raise ValueError(f"{path} is not a JPEG")
    offset = 2
    while offset + 4 <= len(data):
        if data[offset] != 0xFF:
            offset += 1
            continue
        marker = data[offset + 1]
        offset += 2
        while marker == 0xFF and offset < len(data):
            marker = data[offset]
            offset += 1
        if marker in {0xD8, 0xD9}:
            continue
        if offset + 2 > len(data):
            break
        length = int.from_bytes(data[offset : offset + 2], "big")
        if length < 2 or offset + length > len(data):
            break
        if marker in {
            0xC0,
            0xC1,
            0xC2,
            0xC3,
            0xC5,
            0xC6,
            0xC7,
            0xC9,
            0xCA,
            0xCB,
            0xCD,
            0xCE,
            0xCF,
        }:
            if length < 8:
                break
            height = int.from_bytes(data[offset + 3 : offset + 5], "big")
            width = int.from_bytes(data[offset + 5 : offset + 7], "big")
            return width, height
        offset += length
    raise ValueError(f"{path} has no JPEG start-of-frame segment")


def main() -> int:
    manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    assert manifest["schema_version"] == "rustqr.external-negative-corpus.v1"
    assert manifest["name"] == "wikimedia-commons-negative-photos"
    assert manifest["source_repository"] == "https://commons.wikimedia.org"
    assert manifest["source_commit"] == "1131317639"
    assert manifest["source_license"] == "CC-BY-SA-4.0"
    for key in ("license_file", "notice_file", "reuse_declaration"):
        assert (CORPUS_ROOT / manifest[key]).is_file(), f"missing {key}: {manifest[key]}"

    cases = manifest["cases"]
    assert len(cases) == 1, f"expected one Wikimedia fixture, found {len(cases)}"
    case = cases[0]
    assert case["id"] == "wikimedia-computer-screen-monitor"
    assert case["expected_qr_count"] == 0
    assert case["category"] == "photographed_screen"
    assert case["author"] == "U3211603"
    assert case["source_page"].endswith("oldid=1131317639")
    assert case["source_path"].startswith("https://upload.wikimedia.org/")
    assert len(case["source_sha1"]) == 40
    assert case["manual_zero_qr_annotation"]
    path = CORPUS_ROOT / case["local_path"]
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    assert digest == case["sha256"], "SHA-256 mismatch"
    assert jpeg_dimensions(path) == (case["width"], case["height"]), "dimension mismatch"
    pixels = case["width"] * case["height"]
    print(
        "WIKIMEDIA_CORPUS_INTEGRITY "
        f"cases=1 pixels={pixels} megapixels={pixels / 1_000_000:.6f}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

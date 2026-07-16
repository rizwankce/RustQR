#!/usr/bin/env python3
"""Verify the pinned multi-asset Wikimedia Commons negative corpus."""

from __future__ import annotations

import hashlib
import json
import struct
import string
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CORPUS_ROOT = ROOT / "tests" / "negative_corpus"
MANIFEST_PATH = CORPUS_ROOT / "wikimedia_commons_manifest.json"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


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
            0xC0, 0xC1, 0xC2, 0xC3, 0xC5, 0xC6, 0xC7,
            0xC9, 0xCA, 0xCB, 0xCD, 0xCE, 0xCF,
        }:
            if length < 8:
                break
            height = int.from_bytes(data[offset + 3 : offset + 5], "big")
            width = int.from_bytes(data[offset + 5 : offset + 7], "big")
            return width, height
        offset += length
    raise ValueError(f"{path} has no JPEG start-of-frame segment")


def png_dimensions(path: Path) -> tuple[int, int]:
    data = path.read_bytes()
    if data[:8] != PNG_SIGNATURE or data[12:16] != b"IHDR":
        raise ValueError(f"{path} is not a PNG with an IHDR header")
    return struct.unpack(">II", data[16:24])


def image_dimensions(path: Path) -> tuple[int, int]:
    if path.suffix.lower() in {".jpg", ".jpeg"}:
        return jpeg_dimensions(path)
    if path.suffix.lower() == ".png":
        return png_dimensions(path)
    raise ValueError(f"unsupported Wikimedia fixture type: {path}")


def verify_manifest(manifest_path: Path = MANIFEST_PATH, corpus_root: Path = CORPUS_ROOT) -> tuple[int, int]:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    assert manifest["schema_version"] == "rustqr.wikimedia-negative-corpus.v2"
    assert manifest["name"] == "wikimedia-commons-negative-photos"
    assert manifest["source_repository"] == "https://commons.wikimedia.org"
    cases = manifest["cases"]
    assert cases, "Wikimedia corpus must not be empty"
    seen_ids: set[str] = set()
    seen_paths: set[str] = set()
    total_pixels = 0
    for case in cases:
        assert case["id"] not in seen_ids, f"duplicate id: {case['id']}"
        assert case["local_path"] not in seen_paths, f"duplicate path: {case['local_path']}"
        seen_ids.add(case["id"])
        seen_paths.add(case["local_path"])
        assert case["expected_qr_count"] == 0, f"not a negative: {case['id']}"
        assert case["category"].startswith("photographed_"), f"unexpected category: {case['id']}"
        assert case["source_path"].startswith("https://upload.wikimedia.org/")
        assert case["source_page"].startswith("https://commons.wikimedia.org/")
        assert case["source_page"].endswith(f"oldid={case['source_revision']}")
        assert len(case["source_sha1"]) == 40
        assert all(character in string.hexdigits for character in case["source_sha1"])
        assert case["author"]
        assert case["manual_zero_qr_annotation"]
        assert case["source_license"].startswith("CC-BY")
        for key in ("license_file", "notice_file", "reuse_declaration"):
            assert (corpus_root / case[key]).is_file(), f"missing {key}: {case[key]}"
        path = corpus_root / case["local_path"]
        assert hashlib.sha256(path.read_bytes()).hexdigest() == case["sha256"], (
            f"SHA-256 mismatch: {case['id']}"
        )
        dimensions = image_dimensions(path)
        assert dimensions == (case["width"], case["height"]), (
            f"dimension mismatch: {case['id']}: {dimensions}"
        )
        total_pixels += case["width"] * case["height"]
    return len(cases), total_pixels


def main() -> int:
    cases, pixels = verify_manifest()
    print(
        "WIKIMEDIA_CORPUS_INTEGRITY "
        f"cases={cases} pixels={pixels} megapixels={pixels / 1_000_000:.6f}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

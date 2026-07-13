#!/usr/bin/env python3
"""Verify the pinned ZXing negative-corpus assets before evaluating them."""

from __future__ import annotations

import hashlib
import json
import struct
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CORPUS_ROOT = ROOT / "tests" / "negative_corpus"
MANIFEST_PATH = CORPUS_ROOT / "zxing_manifest.json"
PINNED_COMMIT = "82333b3ed894ef097d41dd8c922689ede8880e01"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def png_dimensions(path: Path) -> tuple[int, int]:
    data = path.read_bytes()
    if data[:8] != PNG_SIGNATURE or data[12:16] != b"IHDR":
        raise ValueError(f"{path} is not a PNG with an IHDR header")
    return struct.unpack(">II", data[16:24])


def main() -> int:
    manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    assert manifest["schema_version"] == "rustqr.external-negative-corpus.v1"
    assert manifest["source_commit"] == PINNED_COMMIT
    assert manifest["source_license"] == "Apache-2.0"
    for key in ("license_file", "notice_file", "reuse_declaration"):
        assert (CORPUS_ROOT / manifest[key]).is_file(), f"missing {key}: {manifest[key]}"

    cases = manifest["cases"]
    assert len(cases) == 47, f"expected 47 assets, found {len(cases)}"
    seen_ids: set[str] = set()
    seen_paths: set[str] = set()
    total_pixels = 0
    for case in cases:
        assert case["id"] not in seen_ids, f"duplicate id: {case['id']}"
        assert case["local_path"] not in seen_paths, f"duplicate path: {case['local_path']}"
        seen_ids.add(case["id"])
        seen_paths.add(case["local_path"])
        assert case["expected_qr_count"] == 0, f"not a negative: {case['id']}"
        assert case["source_path"].startswith(
            "core/src/test/resources/blackbox/"
        ), f"unexpected source path: {case['source_path']}"
        path = CORPUS_ROOT / case["local_path"]
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        assert digest == case["sha256"], f"SHA-256 mismatch: {case['id']}"
        dimensions = png_dimensions(path)
        assert dimensions == (case["width"], case["height"]), (
            f"dimension mismatch: {case['id']}: {dimensions}"
        )
        total_pixels += case["width"] * case["height"]

    print(
        "ZXING_CORPUS_INTEGRITY "
        f"cases={len(cases)} pixels={total_pixels} megapixels={total_pixels / 1_000_000:.6f}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

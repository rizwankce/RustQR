#!/usr/bin/env python3
"""Verify the pinned Wikimedia Commons book-shelf negative fixture."""

from __future__ import annotations

import hashlib
import json
import struct
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CORPUS_ROOT = ROOT / "tests" / "negative_corpus"
MANIFEST_PATH = CORPUS_ROOT / "wikimedia_bookshelf_manifest.json"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def png_dimensions(path: Path) -> tuple[int, int]:
    data = path.read_bytes()
    if data[:8] != PNG_SIGNATURE or data[12:16] != b"IHDR":
        raise ValueError(f"{path} is not a PNG with an IHDR header")
    return struct.unpack(">II", data[16:24])


def main() -> int:
    manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    assert manifest["schema_version"] == "rustqr.external-negative-corpus.v1"
    assert manifest["name"] == "wikimedia-commons-negative-bookshelf-photo"
    assert manifest["source_repository"] == "https://commons.wikimedia.org"
    assert manifest["source_commit"] == "828586508"
    assert manifest["source_license"] == "CC-BY-3.0"
    for key in ("license_file", "notice_file", "reuse_declaration"):
        assert (CORPUS_ROOT / manifest[key]).is_file(), f"missing {key}: {manifest[key]}"
    cases = manifest["cases"]
    assert len(cases) == 1, f"expected one book-shelf fixture, found {len(cases)}"
    case = cases[0]
    assert case["id"] == "wikimedia-book-shelf-use"
    assert case["expected_qr_count"] == 0
    assert case["category"] == "photographed_text"
    assert case["author"] == "Valdes-and-Rauber"
    assert case["source_page"].endswith("oldid=828586508")
    assert case["source_path"].startswith("https://upload.wikimedia.org/")
    assert len(case["source_sha1"]) == 40
    assert case["manual_zero_qr_annotation"]
    path = CORPUS_ROOT / case["local_path"]
    assert hashlib.sha256(path.read_bytes()).hexdigest() == case["sha256"], "SHA-256 mismatch"
    assert png_dimensions(path) == (case["width"], case["height"]), "dimension mismatch"
    pixels = case["width"] * case["height"]
    print(
        "WIKIMEDIA_BOOKSHELF_CORPUS_INTEGRITY "
        f"cases=1 pixels={pixels} megapixels={pixels / 1_000_000:.6f}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

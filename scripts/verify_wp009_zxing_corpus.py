#!/usr/bin/env python3
"""Verify the pinned ZXing negative-corpus assets before evaluating them."""

from __future__ import annotations

import hashlib
import json
import struct
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CORPUS_ROOT = ROOT / "tests" / "negative_corpus"
MANIFEST_PATHS = (
    CORPUS_ROOT / "zxing_manifest.json",
    CORPUS_ROOT / "zxing_cross_symbology_manifest.json",
)
PINNED_COMMIT = "82333b3ed894ef097d41dd8c922689ede8880e01"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
EXPECTED_CROSS_SYMBOLOGY_SUITES = {
    "valid_aztec": ("AZTEC", "aztec-1", "zxing_cross_symbology/aztec/", 17),
    "valid_datamatrix": (
        "DATA_MATRIX",
        "datamatrix-1",
        "zxing_cross_symbology/datamatrix/",
        23,
    ),
    "valid_code128": (
        "CODE_128",
        "code128-1",
        "zxing_cross_symbology/code128/",
        7,
    ),
}


def png_dimensions(path: Path) -> tuple[int, int]:
    data = path.read_bytes()
    if data[:8] != PNG_SIGNATURE or data[12:16] != b"IHDR":
        raise ValueError(f"{path} is not a PNG with an IHDR header")
    return struct.unpack(">II", data[16:24])


def verify_manifest(path: Path) -> tuple[int, int]:
    manifest = json.loads(path.read_text(encoding="utf-8"))
    assert manifest["schema_version"] == "rustqr.external-negative-corpus.v1"
    assert manifest["source_commit"] == PINNED_COMMIT
    assert manifest["source_license"] == "Apache-2.0"
    for key in ("license_file", "notice_file", "reuse_declaration"):
        assert (CORPUS_ROOT / manifest[key]).is_file(), f"missing {key}: {manifest[key]}"

    cases = manifest["cases"]
    expected_cases = 47
    assert len(cases) == expected_cases, (
        f"{manifest['name']}: expected {expected_cases} assets, found {len(cases)}"
    )
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

    if manifest["name"] == "zxing-cross-symbology-blackbox":
        suite_counts = {category: 0 for category in EXPECTED_CROSS_SYMBOLOGY_SUITES}
        for case in cases:
            category = case["category"]
            assert category in EXPECTED_CROSS_SYMBOLOGY_SUITES, (
                f"unexpected cross-symbology category: {category}"
            )
            expected_format, source_directory, local_prefix, _ = (
                EXPECTED_CROSS_SYMBOLOGY_SUITES[category]
            )
            assert case.get("source_expected_format") == expected_format, (
                f"unexpected source format: {case['id']}"
            )
            assert case["source_path"].startswith(
                f"core/src/test/resources/blackbox/{source_directory}/"
            ), f"unexpected source suite: {case['id']}"
            assert case["local_path"].startswith(local_prefix), (
                f"unexpected local suite: {case['id']}"
            )
            suite_counts[category] += 1
        for category, (_, _, _, expected_count) in EXPECTED_CROSS_SYMBOLOGY_SUITES.items():
            assert suite_counts[category] == expected_count, (
                f"{category}: expected {expected_count}, found {suite_counts[category]}"
            )

    return len(cases), total_pixels

def main() -> int:
    total_cases = 0
    total_pixels = 0
    for manifest_path in MANIFEST_PATHS:
        cases, pixels = verify_manifest(manifest_path)
        total_cases += cases
        total_pixels += pixels
    print(
        "ZXING_CORPUS_INTEGRITY "
        f"manifests={len(MANIFEST_PATHS)} cases={total_cases} pixels={total_pixels} "
        f"megapixels={total_pixels / 1_000_000:.6f}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

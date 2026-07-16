#!/usr/bin/env python3
"""Run admitted WP-009 negative corpora and emit machine-readable FPRs.

The Rust integration test drives both the self-authored baseline and the
vendored ZXing negative corpus through the public image API. A timeout is
reported independently and is never treated as a zero-detection pass.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

METRICS = re.compile(
    r"(?P<name>(?:(?:ZXING(?:_CROSS_SYMBOLOGY)?|WIKIMEDIA_(?:COMMONS|BOOKSHELF))_)?NEGATIVE_CORPUS_METRICS) cases=(?P<cases>\d+) pixels=(?P<pixels>\d+) "
    r"megapixels=(?P<megapixels>[0-9.]+) positive_images=(?P<positive_images>\d+) "
    r"timeout_images=(?P<timeout_images>\d+) "
    r"false_positive_detections=(?P<false_positive_detections>\d+) "
    r"fp_per_image=(?P<fp_per_image>[0-9.]+) fp_per_megapixel=(?P<fp_per_megapixel>[0-9.]+)"
)


def parse_metrics(
    output: str, expected: set[str]
) -> dict[str, dict[str, int | float]]:
    """Extract every stable corpus metric, rejecting partial or stale output."""
    reports: dict[str, dict[str, int | float]] = {}
    for match in METRICS.finditer(output):
        values = match.groupdict()
        name = values.pop("name").lower().removesuffix("_metrics")
        reports[name] = {
            "cases": int(values["cases"]),
            "pixels": int(values["pixels"]),
            "megapixels": float(values["megapixels"]),
            "positive_images": int(values["positive_images"]),
            "timeout_images": int(values["timeout_images"]),
            "false_positive_detections": int(values["false_positive_detections"]),
            "false_positives_per_image": float(values["fp_per_image"]),
            "false_positives_per_megapixel": float(values["fp_per_megapixel"]),
        }
    if reports.keys() != expected:
        raise ValueError(
            "negative corpus test did not emit both expected metrics: "
            f"expected {sorted(expected)}, got {sorted(reports)}"
        )
    return reports


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, help="write JSON report to this path")
    parser.add_argument(
        "--include-cross-symbology",
        action="store_true",
        help="run the strict ignored cross-symbology qualification gate",
    )
    parser.add_argument(
        "--include-wikimedia",
        action="store_true",
        help="run the strict ignored release-qualified Wikimedia photo gate",
    )
    args = parser.parse_args()
    integrity = subprocess.run(
        [sys.executable, "scripts/verify_wp009_zxing_corpus.py"],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    sys.stdout.write(integrity.stdout)
    if integrity.returncode:
        return integrity.returncode
    wikimedia_integrity = subprocess.run(
        [sys.executable, "scripts/verify_wp009_wikimedia_corpus.py"],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    sys.stdout.write(wikimedia_integrity.stdout)
    if wikimedia_integrity.returncode:
        return wikimedia_integrity.returncode
    bookshelf_integrity = subprocess.run(
        [sys.executable, "scripts/verify_wp009_wikimedia_bookshelf_corpus.py"],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    sys.stdout.write(bookshelf_integrity.stdout)
    if bookshelf_integrity.returncode:
        return bookshelf_integrity.returncode
    command = [
        "cargo", "test", "--test", "negative_image_corpus_tests",
        "--all-features", "--", "--nocapture",
    ]
    result = subprocess.run(
        command,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    sys.stdout.write(result.stdout)
    if result.returncode:
        return result.returncode
    output = result.stdout
    expected = {"negative_corpus", "zxing_negative_corpus"}
    if args.include_cross_symbology:
        cross_command = [
            "cargo", "test", "--test", "negative_image_corpus_tests",
            "--all-features", "admitted_zxing_cross_symbology", "--",
            "--ignored", "--nocapture",
        ]
        cross_result = subprocess.run(
            cross_command,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
        )
        sys.stdout.write(cross_result.stdout)
        if cross_result.returncode:
            return cross_result.returncode
        output += cross_result.stdout
        expected.add("zxing_cross_symbology_negative_corpus")
    if args.include_wikimedia:
        wikimedia_command = [
            "cargo", "test", "--release", "--test", "negative_image_corpus_tests",
            "--all-features", "admitted_wikimedia_", "--", "--ignored", "--nocapture",
        ]
        wikimedia_result = subprocess.run(
            wikimedia_command,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
        )
        sys.stdout.write(wikimedia_result.stdout)
        if wikimedia_result.returncode:
            return wikimedia_result.returncode
        output += wikimedia_result.stdout
        expected.add("wikimedia_commons_negative_corpus")
        expected.add("wikimedia_bookshelf_negative_corpus")
    report = {
        "schema_version": "rustqr.negative-corpus-report.v1",
        "integrity_commands": [
            f"{sys.executable} scripts/verify_wp009_zxing_corpus.py",
            f"{sys.executable} scripts/verify_wp009_wikimedia_corpus.py",
            f"{sys.executable} scripts/verify_wp009_wikimedia_bookshelf_corpus.py",
        ],
        "command": " ".join(command),
        "corpora": parse_metrics(output, expected),
    }
    encoded = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.write_text(encoded, encoding="utf-8")
    else:
        print(encoded, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

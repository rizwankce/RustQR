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
    r"(?P<name>(?:ZXING_)?NEGATIVE_CORPUS_METRICS) cases=(?P<cases>\d+) pixels=(?P<pixels>\d+) "
    r"megapixels=(?P<megapixels>[0-9.]+) positive_images=(?P<positive_images>\d+) "
    r"timeout_images=(?P<timeout_images>\d+) "
    r"false_positive_detections=(?P<false_positive_detections>\d+) "
    r"fp_per_image=(?P<fp_per_image>[0-9.]+) fp_per_megapixel=(?P<fp_per_megapixel>[0-9.]+)"
)


def parse_metrics(output: str) -> dict[str, dict[str, int | float]]:
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
    expected = {"negative_corpus", "zxing_negative_corpus"}
    if reports.keys() != expected:
        raise ValueError(
            "negative corpus test did not emit both expected metrics: "
            f"expected {sorted(expected)}, got {sorted(reports)}"
        )
    return reports


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, help="write JSON report to this path")
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
    result = subprocess.run(
        ["cargo", "test", "--test", "negative_image_corpus_tests", "--all-features", "--", "--nocapture"],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    sys.stdout.write(result.stdout)
    if result.returncode:
        return result.returncode
    report = {
        "schema_version": "rustqr.negative-corpus-report.v1",
        "integrity_command": f"{sys.executable} scripts/verify_wp009_zxing_corpus.py",
        "command": "cargo test --test negative_image_corpus_tests --all-features -- --nocapture",
        "corpora": parse_metrics(result.stdout),
    }
    encoded = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.write_text(encoded, encoding="utf-8")
    else:
        print(encoded, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

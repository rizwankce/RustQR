#!/usr/bin/env python3
"""Run the deterministic WP-009 negative corpus and emit a machine-readable FPR.

The corpus is source-generated and self-authored; see
``tests/negative_corpus/manifest.json``. This wrapper captures the one-line
metric emitted by the Rust integration test and writes a report suitable for
before/after recovery-change comparisons.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

METRICS = re.compile(
    r"NEGATIVE_CORPUS_METRICS cases=(?P<cases>\d+) pixels=(?P<pixels>\d+) "
    r"megapixels=(?P<megapixels>[0-9.]+) positive_images=(?P<positive_images>\d+) "
    r"timeout_images=(?P<timeout_images>\d+) "
    r"false_positive_detections=(?P<false_positive_detections>\d+) "
    r"fp_per_image=(?P<fp_per_image>[0-9.]+) fp_per_megapixel=(?P<fp_per_megapixel>[0-9.]+)"
)


def parse_metrics(output: str) -> dict[str, int | float]:
    """Extract the stable test metric, rejecting partial or stale output."""
    match = METRICS.search(output)
    if match is None:
        raise ValueError("negative corpus test did not emit NEGATIVE_CORPUS_METRICS")
    values = match.groupdict()
    return {
        "cases": int(values["cases"]),
        "pixels": int(values["pixels"]),
        "megapixels": float(values["megapixels"]),
        "positive_images": int(values["positive_images"]),
        "timeout_images": int(values["timeout_images"]),
        "false_positive_detections": int(values["false_positive_detections"]),
        "false_positives_per_image": float(values["fp_per_image"]),
        "false_positives_per_megapixel": float(values["fp_per_megapixel"]),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, help="write JSON report to this path")
    args = parser.parse_args()
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
        "command": "cargo test --test negative_image_corpus_tests --all-features -- --nocapture",
        "metrics": parse_metrics(result.stdout),
    }
    encoded = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.write_text(encoded, encoding="utf-8")
    else:
        print(encoded, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

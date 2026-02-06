#!/usr/bin/env python3
"""Compare RustQR reading-rate benchmark artifacts.

Fails when:
- Weighted global reading-rate drops by more than threshold percentage points.
- Median per-image runtime regresses by more than threshold percent.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path


def load_artifact(path: Path) -> dict:
    try:
        with path.open("r", encoding="utf-8") as f:
            data = json.load(f)
    except FileNotFoundError as exc:
        raise ValueError(f"artifact not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid JSON in {path}: {exc}") from exc

    if "summary" not in data:
        raise ValueError(f"missing summary in {path}")
    return data


def read_metrics(data: dict, path: Path) -> tuple[float, float]:
    summary = data.get("summary", {})
    runtime = summary.get("runtime", {})

    weighted = summary.get("weighted_global_rate_percent")
    median_ms = runtime.get("median_per_image_ms")

    if not isinstance(weighted, (int, float)):
        raise ValueError(f"missing numeric summary.weighted_global_rate_percent in {path}")
    if not isinstance(median_ms, (int, float)):
        raise ValueError(f"missing numeric summary.runtime.median_per_image_ms in {path}")

    return float(weighted), float(median_ms)


def read_fingerprint(data: dict, path: Path) -> str:
    metadata = data.get("metadata", {})
    fingerprint = metadata.get("dataset_fingerprint")
    if not isinstance(fingerprint, str) or not fingerprint:
        raise ValueError(f"missing metadata.dataset_fingerprint in {path}")
    return fingerprint


def per_category_rates(data: dict) -> dict[str, float]:
    out: dict[str, float] = {}
    for entry in data.get("categories", []):
        if not isinstance(entry, dict):
            continue
        name = entry.get("name")
        rate = entry.get("rate_percent")
        if isinstance(name, str) and isinstance(rate, (int, float)):
            out[name] = float(rate)
    return out


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Compare reading-rate artifacts and fail if weighted-global rate drops "
            "or median runtime regresses beyond configured thresholds."
        )
    )
    parser.add_argument("--baseline", required=True, help="Path to baseline artifact JSON")
    parser.add_argument("--candidate", required=True, help="Path to candidate artifact JSON")
    parser.add_argument(
        "--max-rate-drop-pp",
        type=float,
        default=1.0,
        help="Maximum allowed weighted-global rate drop in percentage points (default: 1.0)",
    )
    parser.add_argument(
        "--max-median-runtime-regression-pct",
        type=float,
        default=15.0,
        help="Maximum allowed median per-image runtime regression percent (default: 15.0)",
    )
    parser.add_argument(
        "--allow-dataset-mismatch",
        action="store_true",
        help="Allow comparing artifacts with different dataset fingerprints",
    )

    args = parser.parse_args()

    baseline_path = Path(args.baseline)
    candidate_path = Path(args.candidate)

    try:
        baseline = load_artifact(baseline_path)
        candidate = load_artifact(candidate_path)
        baseline_rate, baseline_median = read_metrics(baseline, baseline_path)
        candidate_rate, candidate_median = read_metrics(candidate, candidate_path)
        baseline_fp = read_fingerprint(baseline, baseline_path)
        candidate_fp = read_fingerprint(candidate, candidate_path)
    except ValueError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2

    if baseline_fp != candidate_fp and not args.allow_dataset_mismatch:
        print(
            "FAIL: dataset fingerprint mismatch "
            f"(baseline={baseline_fp}, candidate={candidate_fp})"
        )
        print("Use --allow-dataset-mismatch only for exploratory comparisons.")
        return 1

    rate_drop_pp = baseline_rate - candidate_rate
    if baseline_median <= 0:
        runtime_regression_pct = 0.0 if candidate_median <= 0 else math.inf
    else:
        runtime_regression_pct = ((candidate_median - baseline_median) / baseline_median) * 100.0

    print("RustQR reading-rate A/B comparison")
    print(f"Baseline:  {baseline_path}")
    print(f"Candidate: {candidate_path}")
    print(f"Dataset fingerprint: baseline={baseline_fp} candidate={candidate_fp}")
    print(
        f"Weighted-global rate: baseline={baseline_rate:.4f}% candidate={candidate_rate:.4f}% "
        f"drop={rate_drop_pp:.4f} pp"
    )
    print(
        f"Median runtime: baseline={baseline_median:.4f} ms candidate={candidate_median:.4f} ms "
        f"regression={runtime_regression_pct:.2f}%"
    )

    baseline_categories = per_category_rates(baseline)
    candidate_categories = per_category_rates(candidate)
    shared = sorted(set(baseline_categories) & set(candidate_categories))
    if shared:
        print("Per-category deltas (candidate - baseline, percentage points):")
        for name in shared:
            delta = candidate_categories[name] - baseline_categories[name]
            print(
                f"  {name:16s} baseline={baseline_categories[name]:7.2f}% "
                f"candidate={candidate_categories[name]:7.2f}% delta={delta:+7.2f} pp"
            )

    failed = False
    if rate_drop_pp > args.max_rate_drop_pp:
        print(
            "FAIL: weighted-global rate drop "
            f"{rate_drop_pp:.4f} pp exceeds {args.max_rate_drop_pp:.4f} pp"
        )
        failed = True
    if runtime_regression_pct > args.max_median_runtime_regression_pct:
        print(
            "FAIL: median runtime regression "
            f"{runtime_regression_pct:.2f}% exceeds "
            f"{args.max_median_runtime_regression_pct:.2f}%"
        )
        failed = True

    if failed:
        return 1

    print("PASS: thresholds satisfied")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

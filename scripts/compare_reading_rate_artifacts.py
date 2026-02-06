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
from typing import Dict, List, Tuple


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


def per_category_hits_totals(data: dict) -> dict[str, Tuple[int, int]]:
    out: dict[str, Tuple[int, int]] = {}
    for entry in data.get("categories", []):
        if not isinstance(entry, dict):
            continue
        name = entry.get("name")
        hits = entry.get("hits")
        total = entry.get("total_expected")
        if isinstance(name, str) and isinstance(hits, int) and isinstance(total, int):
            out[name] = (hits, total)
    return out


def parse_category_thresholds(values: List[str]) -> Dict[str, float]:
    out: Dict[str, float] = {}
    for raw in values:
        if "=" not in raw:
            raise ValueError(f"invalid category threshold '{raw}', expected name=max_drop_pp")
        name, threshold = raw.split("=", 1)
        name = name.strip()
        try:
            parsed = float(threshold.strip())
        except ValueError as exc:
            raise ValueError(
                f"invalid threshold for category '{name}' in '{raw}'"
            ) from exc
        if not name:
            raise ValueError(f"empty category name in '{raw}'")
        out[name] = parsed
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
    parser.add_argument(
        "--category-max-drop-pp",
        action="append",
        default=[],
        metavar="NAME=DROP",
        help=(
            "Per-category max allowed rate drop in percentage points, "
            "repeatable (example: lots=2.0)"
        ),
    )
    parser.add_argument(
        "--contribution-report",
        default="",
        help="Optional output path for per-category weighted contribution JSON report",
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
        category_thresholds = parse_category_thresholds(args.category_max_drop_pp)
    except ValueError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2

    if not category_thresholds:
        category_thresholds = {
            "lots": 2.0,
            "rotations": 2.0,
            "nominal": 1.5,
            "high_version": 1.5,
        }

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
    baseline_hits = per_category_hits_totals(baseline)
    candidate_hits = per_category_hits_totals(candidate)
    shared = sorted(set(baseline_categories) & set(candidate_categories))
    if shared:
        print("Per-category deltas (candidate - baseline, percentage points):")
        for name in shared:
            delta = candidate_categories[name] - baseline_categories[name]
            print(
                f"  {name:16s} baseline={baseline_categories[name]:7.2f}% "
                f"candidate={candidate_categories[name]:7.2f}% delta={delta:+7.2f} pp"
            )

    baseline_total_expected = baseline.get("summary", {}).get("total_expected", 0)
    if not isinstance(baseline_total_expected, int) or baseline_total_expected <= 0:
        baseline_total_expected = 0

    contributions = []
    if baseline_total_expected > 0:
        for name in sorted(set(baseline_hits) & set(candidate_hits)):
            base_h, base_t = baseline_hits[name]
            cand_h, cand_t = candidate_hits[name]
            if base_t <= 0 or cand_t <= 0:
                continue
            delta_hits = cand_h - base_h
            contribution_pp = (delta_hits / baseline_total_expected) * 100.0
            contributions.append(
                {
                    "category": name,
                    "baseline_hits": base_h,
                    "candidate_hits": cand_h,
                    "delta_hits": delta_hits,
                    "estimated_weighted_global_contribution_pp": contribution_pp,
                }
            )
        contributions.sort(
            key=lambda row: (
                -abs(row["estimated_weighted_global_contribution_pp"]),
                row["category"],
            )
        )
        if contributions:
            print("Estimated weighted-global contribution by category (pp):")
            for row in contributions:
                print(
                    "  {category:16s} delta_hits={delta_hits:+4d} contribution={estimated_weighted_global_contribution_pp:+7.3f}".format(
                        **row
                    )
                )

    failed = False
    for name, max_drop in sorted(category_thresholds.items()):
        if name not in baseline_categories or name not in candidate_categories:
            print(f"FAIL: required category '{name}' missing in artifacts")
            failed = True
            continue
        drop = baseline_categories[name] - candidate_categories[name]
        if drop > max_drop:
            print(
                f"FAIL: category '{name}' drop {drop:.4f} pp exceeds {max_drop:.4f} pp"
            )
            failed = True

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

    if args.contribution_report:
        report_path = Path(args.contribution_report)
        report = {
            "baseline": str(baseline_path),
            "candidate": str(candidate_path),
            "dataset_fingerprint": {
                "baseline": baseline_fp,
                "candidate": candidate_fp,
            },
            "weighted_global": {
                "baseline_rate_percent": baseline_rate,
                "candidate_rate_percent": candidate_rate,
                "delta_pp": candidate_rate - baseline_rate,
            },
            "category_thresholds": category_thresholds,
            "contributions": contributions,
        }
        report_path.parent.mkdir(parents=True, exist_ok=True)
        with report_path.open("w", encoding="utf-8") as f:
            json.dump(report, f, indent=2, sort_keys=True)
        print(f"Contribution report: {report_path}")

    print("PASS: thresholds satisfied")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

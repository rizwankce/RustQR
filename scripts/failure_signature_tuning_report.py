#!/usr/bin/env python3
"""Generate failure-signature tuning queue from baseline/candidate artifacts."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

SIGNATURE_HINTS = {
    "no-finders": ("detector/finder+binarization", "QR_MAX_DIM, binarization policy, finder edge thresholds"),
    "no-groups": ("detector/grouping", "group size ratio, geometry rerank weights"),
    "transform-fail": ("transform/sampling", "transform refinement, timing validation thresholds"),
    "format-fail": ("format/sampling", "format BCH tolerance, sampling scale"),
    "rs-fail": ("reed-solomon", "erasure thresholds, max erasures"),
    "payload-fail": ("payload parser", "beam repair knobs, mode gating"),
    "over-budget-skip": ("budget controller", "image attempt cap, lane split"),
    "unknown-fail": ("mixed", "inspect per-image telemetry"),
}


def load_json(path: Path) -> dict:
    try:
        return json.loads(path.read_text())
    except FileNotFoundError as exc:
        raise ValueError(f"missing artifact: {path}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid JSON artifact {path}: {exc}") from exc


def failure_map(artifact: dict) -> dict[str, dict]:
    out: dict[str, dict] = {}
    for row in artifact.get("failure_clusters", []):
        sig = str(row.get("signature", "unknown-fail"))
        out[sig] = {
            "count": int(row.get("count", 0)),
            "qr_weight": int(row.get("qr_weight", 0)),
            "examples": [str(x) for x in row.get("examples", [])],
        }
    return out


def category_rate_map(artifact: dict) -> dict[str, float]:
    out: dict[str, float] = {}
    for row in artifact.get("categories", []):
        name = row.get("name")
        rate = row.get("rate_percent")
        if isinstance(name, str) and isinstance(rate, (int, float)):
            out[name] = float(rate)
    return out


def build_queue(baseline: dict, candidate: dict, top_n: int) -> list[dict]:
    base = failure_map(baseline)
    cand = failure_map(candidate)
    signatures = sorted(set(base) | set(cand))
    rows = []
    for sig in signatures:
        b = base.get(sig, {"count": 0, "qr_weight": 0, "examples": []})
        c = cand.get(sig, {"count": 0, "qr_weight": 0, "examples": []})
        subsystem, knobs = SIGNATURE_HINTS.get(sig, ("mixed", "inspect telemetry"))
        rows.append(
            {
                "signature": sig,
                "baseline_count": b["count"],
                "candidate_count": c["count"],
                "delta_count": c["count"] - b["count"],
                "baseline_qr_weight": b["qr_weight"],
                "candidate_qr_weight": c["qr_weight"],
                "delta_qr_weight": c["qr_weight"] - b["qr_weight"],
                "likely_subsystem": subsystem,
                "proposed_knobs": knobs,
                "example": (c["examples"] or b["examples"] or ["-"])[0],
            }
        )
    rows.sort(
        key=lambda r: (
            -abs(r["delta_qr_weight"]),
            -abs(r["delta_count"]),
            r["signature"],
        )
    )
    return rows[:top_n]


def render_markdown(queue: list[dict], baseline: Path, candidate: Path) -> str:
    lines = [
        "# Failure Signature Tuning Queue",
        "",
        f"Baseline: `{baseline}`",
        f"Candidate: `{candidate}`",
        "",
        "| Rank | Signature | Delta QR Weight | Delta Count | Subsystem | Proposed Knobs | Example |",
        "|------|-----------|-----------------|-------------|-----------|----------------|---------|",
    ]
    for idx, row in enumerate(queue, start=1):
        lines.append(
            f"| {idx} | {row['signature']} | {row['delta_qr_weight']:+d} | {row['delta_count']:+d} | "
            f"{row['likely_subsystem']} | {row['proposed_knobs']} | {row['example']} |"
        )
    lines.append("")
    lines.append("## Merge Criteria")
    lines.append("1. Accept only changes with net weighted-global gain and no gate regressions.")
    lines.append("2. Require at least one top-ranked signature to improve in QR weight.")
    lines.append("3. Re-run compare script to verify runtime guardrail remains satisfied.")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Build signature-level before/after tuning queue from reading-rate artifacts."
    )
    parser.add_argument("--baseline", required=True, help="Baseline artifact JSON")
    parser.add_argument("--candidate", required=True, help="Candidate artifact JSON")
    parser.add_argument("--top-n", type=int, default=8, help="Number of signatures in queue")
    parser.add_argument("--out-json", default="docs/failure_signature_tuning_queue.json")
    parser.add_argument("--out-md", default="docs/failure_signature_tuning_queue.md")
    args = parser.parse_args()

    baseline_path = Path(args.baseline)
    candidate_path = Path(args.candidate)
    baseline = load_json(baseline_path)
    candidate = load_json(candidate_path)

    queue = build_queue(baseline, candidate, max(1, args.top_n))
    baseline_rates = category_rate_map(baseline)
    candidate_rates = category_rate_map(candidate)
    category_deltas = {
        name: candidate_rates.get(name, 0.0) - baseline_rates.get(name, 0.0)
        for name in sorted(set(baseline_rates) | set(candidate_rates))
    }

    report = {
        "schema_version": "rustqr.failure_signature_tuning.v1",
        "baseline": str(baseline_path),
        "candidate": str(candidate_path),
        "queue": queue,
        "category_rate_deltas_pp": category_deltas,
    }

    out_json = Path(args.out_json)
    out_md = Path(args.out_md)
    out_json.parent.mkdir(parents=True, exist_ok=True)
    out_md.parent.mkdir(parents=True, exist_ok=True)
    out_json.write_text(json.dumps(report, indent=2) + "\n")
    out_md.write_text(render_markdown(queue, baseline_path, candidate_path))
    print(f"Wrote: {out_json}")
    print(f"Wrote: {out_md}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())


#!/usr/bin/env python3
"""Cluster and rank RustQR benchmark failure signatures from artifact JSON."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


STAGE_HINTS = {
    "no-finders": "binarization/finder detection",
    "no-groups": "finder grouping / geometry consistency",
    "transform-fail": "transform construction/refinement",
    "format-fail": "sampling quality / format BCH extraction",
    "rs-fail": "Reed-Solomon correction / bitstream quality",
    "payload-fail": "mode parsing / payload integrity",
    "over-budget-skip": "decode budget manager thresholds",
    "unknown-fail": "mixed signals; inspect per-image telemetry",
}


def load_artifact(path: Path) -> dict:
    try:
        return json.loads(path.read_text())
    except FileNotFoundError as exc:
        raise ValueError(f"artifact not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid JSON artifact: {path}: {exc}") from exc


def build_rows(artifact: dict, top_n: int) -> list[dict]:
    rows = artifact.get("failure_clusters", [])
    normalized = []
    for row in rows:
        sig = str(row.get("signature", "unknown-fail"))
        count = int(row.get("count", 0))
        qr_weight = int(row.get("qr_weight", 0))
        examples = [str(x) for x in row.get("examples", [])]
        normalized.append(
            {
                "signature": sig,
                "count": count,
                "qr_weight": qr_weight,
                "examples": examples,
                "stage_hint": STAGE_HINTS.get(sig, "investigate mixed pipeline signals"),
            }
        )

    normalized.sort(
        key=lambda r: (-r["qr_weight"], -r["count"], r["signature"])
    )
    return normalized[:top_n]


def render_markdown(rows: list[dict], artifact_path: Path) -> str:
    lines = [
        "# Failure Cluster Triage Report",
        "",
        f"Source artifact: `{artifact_path}`",
        "",
        "| Rank | Signature | Missed Images | Missed QR Weight | Likely Stage | Example |",
        "|------|-----------|---------------|------------------|--------------|---------|",
    ]

    for i, row in enumerate(rows, start=1):
        example = row["examples"][0] if row["examples"] else "-"
        lines.append(
            f"| {i} | {row['signature']} | {row['count']} | {row['qr_weight']} | "
            f"{row['stage_hint']} | {example} |"
        )

    lines.extend(
        [
            "",
            "## Suggested Actions",
        ]
    )
    for i, row in enumerate(rows, start=1):
        lines.append(
            f"{i}. `{row['signature']}` -> focus on {row['stage_hint']} "
            f"(weight={row['qr_weight']}, misses={row['count']})."
        )

    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Rank failure signatures from a RustQR reading-rate artifact and emit "
            "deterministic JSON + markdown reports."
        )
    )
    parser.add_argument("--artifact", required=True, help="Path to artifact JSON")
    parser.add_argument(
        "--top-n", type=int, default=8, help="Number of top clusters to include"
    )
    parser.add_argument(
        "--out-json",
        default="docs/failure_cluster_report.json",
        help="Output JSON report path",
    )
    parser.add_argument(
        "--out-md",
        default="docs/failure_cluster_report.md",
        help="Output markdown report path",
    )
    args = parser.parse_args()

    artifact_path = Path(args.artifact)
    out_json = Path(args.out_json)
    out_md = Path(args.out_md)

    artifact = load_artifact(artifact_path)
    rows = build_rows(artifact, max(1, args.top_n))

    report = {
        "schema_version": "rustqr.failure_clusters.v1",
        "artifact": str(artifact_path),
        "top_n": max(1, args.top_n),
        "clusters": rows,
    }

    out_json.parent.mkdir(parents=True, exist_ok=True)
    out_json.write_text(json.dumps(report, indent=2) + "\n")
    out_md.parent.mkdir(parents=True, exist_ok=True)
    out_md.write_text(render_markdown(rows, artifact_path))

    print(f"Wrote: {out_json}")
    print(f"Wrote: {out_md}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

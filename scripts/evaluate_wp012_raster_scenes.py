#!/usr/bin/env python3
"""Run QRTool's one-to-one geometry evaluator for each WP-012 density step.

The tool runs each scene in an isolated temporary ``controlled_dense`` category
so QRTool's normal reading-rate path performs the authoritative IoU >= 0.5,
maximum-cardinality bipartite matching.  It emits a compact combined artifact;
the individual temporary artifacts are intentionally discarded because every
field needed to reproduce them is preserved here.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import tempfile
from pathlib import Path


def run_scene(qrtool: Path, scene_dir: Path, stem: str, max_dim: int, timeout_ms: int) -> dict:
    with tempfile.TemporaryDirectory(prefix="rustqr_wp012_") as temporary:
        root = Path(temporary)
        category = root / "controlled_dense"
        category.mkdir()
        for suffix in (".png", ".txt"):
            os.link(scene_dir / f"{stem}{suffix}", category / f"{stem}{suffix}")
        artifact = root / "result.json"
        environment = {**os.environ, "QR_MAX_DIM": str(max_dim)}
        subprocess.run(
            [
                str(qrtool),
                "reading-rate",
                "--root",
                str(root),
                "--category",
                "controlled_dense",
                "--non-interactive",
                "--timeout-ms",
                str(timeout_ms),
                "--artifact-json",
                str(artifact),
            ],
            check=True,
            env=environment,
        )
        result = json.loads(artifact.read_text(encoding="utf-8"))
    return result


def summarize(result: dict, density: int) -> dict:
    summary = result["summary"]
    return {
        "symbols": density,
        "hits": summary["total_hits"],
        "expected": summary["total_expected"],
        "recall": summary["localization_recall"],
        "precision": summary["localization_precision"],
        "false_positives": summary["false_positives"],
        "duplicates": summary["duplicate_predictions"],
        "timeouts": summary["timeouts"],
        "qr_symbols_per_second": summary["qr_symbols_per_second"],
        "end_to_end_ms": summary["end_to_end_runtime"]["mean_per_image_ms"],
        "core_ms": summary["core_runtime"]["mean_per_image_ms"],
        "evaluator_fingerprint": result["metadata"]["evaluator_fingerprint"],
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qrtool", type=Path, default=Path("target/release/qrtool"))
    parser.add_argument("--scenes", type=Path, default=Path("tests/fixtures/wp012_raster_scenes/controlled_dense"))
    parser.add_argument("--max-dim", type=int, default=0)
    parser.add_argument("--timeout-ms", type=int, default=10_000)
    parser.add_argument("--artifact", type=Path, default=Path("artifacts/wp012_controlled_dense_by_density_qrmax0.json"))
    args = parser.parse_args()
    manifest = json.loads((args.scenes.parent / "manifest.json").read_text(encoding="utf-8"))
    rows = []
    for scene in manifest["scenes"]:
        result = run_scene(args.qrtool, args.scenes, scene["id"], args.max_dim, args.timeout_ms)
        rows.append(summarize(result, scene["symbols"]))
    artifact = {
        "schema_version": 1,
        "purpose": "WP-012 density-by-density end-to-end localization evidence",
        "scene_manifest": str(args.scenes.parent / "manifest.json"),
        "scene_manifest_qrcode_backend": manifest["qrcode_backend"],
        "max_dim": args.max_dim,
        "timeout_ms": args.timeout_ms,
        "matching": "qrtool reading-rate localization; quad-iou=0.5; max-cardinality bipartite",
        "results": rows,
    }
    args.artifact.parent.mkdir(parents=True, exist_ok=True)
    args.artifact.write_text(json.dumps(artifact, indent=2) + "\n", encoding="utf-8")
    for row in rows:
        print(
            f"{row['symbols']:>3} symbols: {row['hits']}/{row['expected']} "
            f"({row['recall'] * 100:.2f}%), {row['end_to_end_ms']:.2f} ms, "
            f"{row['qr_symbols_per_second']:.2f} symbols/s"
        )


if __name__ == "__main__":
    main()

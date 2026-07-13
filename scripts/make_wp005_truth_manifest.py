#!/usr/bin/env python3
"""Build a shared WP-005 localization-truth manifest from a prediction export."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path


PREDICTION_SCHEMA = "rustqr.wp005.prediction-stream.v1"
TRUTH_SCHEMA = "rustqr.wp005.localization-truth.v1"
NUMBER = re.compile(r"[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?")


def parse_sets(path: Path) -> list[list[list[float]]]:
    points: list[tuple[float, float]] = []
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or not stripped[0] in "-+.0123456789":
            continue
        values = [float(value) for value in NUMBER.findall(line)]
        if not values:
            continue
        if len(values) % 2:
            raise ValueError(f"{path}: odd coordinate count")
        points.extend(zip(values[::2], values[1::2]))
    if len(points) % 4:
        raise ValueError(f"{path}: {len(points)} points is not a quadrilateral count")
    return [[[x, y] for x, y in points[index:index + 4]] for index in range(0, len(points), 4)]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--predictions", required=True, type=Path)
    parser.add_argument("--dataset-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    prediction = json.loads(args.predictions.read_text(encoding="utf-8"))
    if prediction.get("schema_version") != PREDICTION_SCHEMA:
        raise SystemExit("ERROR: unsupported prediction stream schema")
    images = []
    label_hash = hashlib.sha256()
    for row in prediction.get("images", []):
        image_id = row.get("image_id")
        if not isinstance(image_id, str):
            raise SystemExit("ERROR: prediction row missing image_id")
        label = (args.dataset_root / image_id).with_suffix(".txt")
        if not label.is_file():
            raise SystemExit(f"ERROR: missing label {label}")
        label_hash.update(image_id.encode("utf-8") + b"\0" + label.read_bytes())
        images.append({
            "image_id": image_id, "category": row["category"],
            "original_width": row["original_width"], "original_height": row["original_height"],
            "working_width": row["working_width"], "working_height": row["working_height"],
            "expected_quadrilaterals": parse_sets(label),
        })
    metadata = prediction["metadata"]
    output = {
        "schema_version": TRUTH_SCHEMA,
        "metadata": {
            "dataset_fingerprint": metadata["dataset_fingerprint"],
            "label_fingerprint": label_hash.hexdigest(),
            "preprocessing_fingerprint": metadata["preprocessing_fingerprint"],
        },
        "images": images,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"WP-005 truth manifest: {len(images)} images -> {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

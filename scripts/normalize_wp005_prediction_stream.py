#!/usr/bin/env python3
"""Normalize a branch-neutral WP-005 prediction export into v2 score fields.

This is deliberately an external, throwaway comparison adapter.  It does not
invoke RustQR or alter any decoder branch.  Adapters on `main`, the rebuild,
and the current branch export predictions in original-image coordinates; this
tool joins those predictions with one shared truth manifest and produces the
subset of ``rustqr.reading_rate.v2`` consumed by the existing comparator.
"""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from statistics import median
from typing import Any


PREDICTION_SCHEMA = "rustqr.wp005.prediction-stream.v1"
TRUTH_SCHEMA = "rustqr.wp005.localization-truth.v1"
OUTPUT_SCHEMA = "rustqr.reading_rate.v2"
EVALUATOR = "mode=localization;quad-iou=0.5;matching=max-cardinality"


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read {path}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{path}: expected JSON object")
    return value


def require_string(mapping: dict[str, Any], key: str, where: str) -> str:
    value = mapping.get(key)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{where}: missing nonempty {key}")
    return value


def require_nonnegative_number(mapping: dict[str, Any], key: str, where: str) -> float:
    value = mapping.get(key)
    if not isinstance(value, (int, float)) or isinstance(value, bool) or value < 0:
        raise ValueError(f"{where}: {key} must be a nonnegative number")
    return float(value)


def quadrilateral(value: Any, where: str) -> list[tuple[float, float]]:
    if not isinstance(value, list) or len(value) != 4:
        raise ValueError(f"{where}: quadrilateral must contain four points")
    points: list[tuple[float, float]] = []
    for index, point in enumerate(value):
        if (
            not isinstance(point, list)
            or len(point) != 2
            or not all(isinstance(coordinate, (int, float)) and not isinstance(coordinate, bool) for coordinate in point)
        ):
            raise ValueError(f"{where}[{index}]: point must be [x, y]")
        points.append((float(point[0]), float(point[1])))
    if abs(polygon_area(points)) <= 1e-9:
        raise ValueError(f"{where}: quadrilateral has zero area")
    return points


def polygon_area(points: list[tuple[float, float]]) -> float:
    return sum(
        points[index][0] * points[(index + 1) % len(points)][1]
        - points[(index + 1) % len(points)][0] * points[index][1]
        for index in range(len(points))
    ) / 2.0


def ensure_ccw(points: list[tuple[float, float]]) -> list[tuple[float, float]]:
    return points if polygon_area(points) > 0 else list(reversed(points))


def inside(point: tuple[float, float], edge_start: tuple[float, float], edge_end: tuple[float, float]) -> bool:
    return ((edge_end[0] - edge_start[0]) * (point[1] - edge_start[1])
            - (edge_end[1] - edge_start[1]) * (point[0] - edge_start[0])) >= -1e-9


def intersection(
    start: tuple[float, float], end: tuple[float, float], edge_start: tuple[float, float], edge_end: tuple[float, float]
) -> tuple[float, float]:
    dx, dy = end[0] - start[0], end[1] - start[1]
    ex, ey = edge_end[0] - edge_start[0], edge_end[1] - edge_start[1]
    denominator = dx * ey - dy * ex
    if abs(denominator) <= 1e-12:
        return end
    factor = ((edge_start[0] - start[0]) * ey - (edge_start[1] - start[1]) * ex) / denominator
    return start[0] + factor * dx, start[1] + factor * dy


def intersection_area(left: list[tuple[float, float]], right: list[tuple[float, float]]) -> float:
    clipped = ensure_ccw(left)
    for index, edge_start in enumerate(ensure_ccw(right)):
        edge_end = ensure_ccw(right)[(index + 1) % len(right)]
        source = clipped
        clipped = []
        if not source:
            break
        previous = source[-1]
        for current in source:
            current_inside = inside(current, edge_start, edge_end)
            previous_inside = inside(previous, edge_start, edge_end)
            if current_inside:
                if not previous_inside:
                    clipped.append(intersection(previous, current, edge_start, edge_end))
                clipped.append(current)
            elif previous_inside:
                clipped.append(intersection(previous, current, edge_start, edge_end))
            previous = current
    return abs(polygon_area(clipped)) if len(clipped) >= 3 else 0.0


def iou(left: list[tuple[float, float]], right: list[tuple[float, float]]) -> float:
    overlap = intersection_area(left, right)
    union = abs(polygon_area(left)) + abs(polygon_area(right)) - overlap
    return overlap / union if union > 0 else 0.0


def score_localizations(
    expected: list[list[tuple[float, float]]], predicted: list[list[tuple[float, float]]]
) -> tuple[int, int]:
    """Maximum-cardinality matching for the fixed 0.5-IoU contract."""
    edges = [[candidate for candidate, quad in enumerate(predicted) if iou(truth, quad) >= 0.5]
             for truth in expected]
    assigned = [-1] * len(predicted)

    def visit(truth_index: int, seen: set[int]) -> bool:
        for candidate in edges[truth_index]:
            if candidate in seen:
                continue
            seen.add(candidate)
            if assigned[candidate] < 0 or visit(assigned[candidate], seen):
                assigned[candidate] = truth_index
                return True
        return False

    hits = sum(visit(index, set()) for index in range(len(expected)))
    matched_predictions = {
        prediction for prediction, truth_index in enumerate(assigned) if truth_index >= 0
    }
    duplicates = sum(
        1
        for prediction, quad in enumerate(predicted)
        if prediction not in matched_predictions
        and any(iou(annotation, quad) >= 0.5 for annotation in expected)
    )
    return hits, duplicates


def percentile(values: list[float], fraction: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    position = (len(ordered) - 1) * fraction
    low = int(position)
    high = min(low + 1, len(ordered) - 1)
    return ordered[low] + (ordered[high] - ordered[low]) * (position - low)


def runtime(values: list[float]) -> dict[str, float | int]:
    return {
        "samples": len(values), "total_ms": sum(values), "mean_per_image_ms": sum(values) / len(values) if values else 0.0,
        "median_per_image_ms": median(values) if values else 0.0,
        "p50_per_image_ms": percentile(values, 0.5), "p90_per_image_ms": percentile(values, 0.9),
        "p95_per_image_ms": percentile(values, 0.95), "p99_per_image_ms": percentile(values, 0.99),
        "min_per_image_ms": min(values) if values else 0.0, "max_per_image_ms": max(values) if values else 0.0,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--predictions", required=True, type=Path)
    parser.add_argument("--truth", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    try:
        predictions = load_json(args.predictions)
        truth = load_json(args.truth)
        if predictions.get("schema_version") != PREDICTION_SCHEMA:
            raise ValueError("predictions: unsupported schema_version")
        if truth.get("schema_version") != TRUTH_SCHEMA:
            raise ValueError("truth: unsupported schema_version")
        metadata = predictions.get("metadata")
        truth_metadata = truth.get("metadata")
        if not isinstance(metadata, dict) or not isinstance(truth_metadata, dict):
            raise ValueError("predictions and truth require metadata objects")
        for key in ("dataset_fingerprint", "preprocessing_fingerprint"):
            if require_string(metadata, key, "predictions.metadata") != require_string(truth_metadata, key, "truth.metadata"):
                raise ValueError(f"metadata mismatch for {key}")
        prediction_rows = predictions.get("images")
        truth_rows = truth.get("images")
        if not isinstance(prediction_rows, list) or not isinstance(truth_rows, list):
            raise ValueError("predictions and truth require images arrays")
        truth_by_id: dict[str, dict[str, Any]] = {}
        for row in truth_rows:
            if not isinstance(row, dict):
                raise ValueError("truth image entry must be an object")
            image_id = require_string(row, "image_id", "truth image")
            if image_id in truth_by_id:
                raise ValueError(f"duplicate truth image_id: {image_id}")
            require_string(row, "category", f"truth {image_id}")
            truth_by_id[image_id] = row
        if len(prediction_rows) != len(truth_by_id):
            raise ValueError("prediction/truth image counts differ")

        categories: dict[str, dict[str, Any]] = {}
        core_times: list[float] = []
        wall_times: list[float] = []
        seen_ids: set[str] = set()
        for row in prediction_rows:
            if not isinstance(row, dict):
                raise ValueError("prediction image entry must be an object")
            image_id = require_string(row, "image_id", "prediction image")
            if image_id in seen_ids or image_id not in truth_by_id:
                raise ValueError(f"unknown or duplicate prediction image_id: {image_id}")
            seen_ids.add(image_id)
            expected_row = truth_by_id[image_id]
            category = require_string(row, "category", f"prediction {image_id}")
            if category != require_string(expected_row, "category", f"truth {image_id}"):
                raise ValueError(f"category mismatch for {image_id}")
            for dimension in ("original_width", "original_height", "working_width", "working_height"):
                if row.get(dimension) != expected_row.get(dimension) or not isinstance(row.get(dimension), int) or row[dimension] <= 0:
                    raise ValueError(f"dimension mismatch or invalid {dimension} for {image_id}")
            expected = [quadrilateral(quad, f"truth {image_id}.expected_quadrilaterals") for quad in expected_row.get("expected_quadrilaterals", [])]
            raw_predicted = row.get("predicted_quadrilaterals")
            if not isinstance(raw_predicted, list):
                raise ValueError(f"prediction {image_id}: predicted_quadrilaterals must be an array")
            predicted = [quadrilateral(quad, f"prediction {image_id}.predicted_quadrilaterals") for quad in raw_predicted]
            timed_out = row.get("timed_out")
            if not isinstance(timed_out, bool):
                raise ValueError(f"prediction {image_id}: timed_out must be boolean")
            core = require_nonnegative_number(row, "core_elapsed_ms", f"prediction {image_id}")
            wall = require_nonnegative_number(row, "end_to_end_elapsed_ms", f"prediction {image_id}")
            core_times.append(core)
            wall_times.append(wall)
            result = categories.setdefault(category, {"name": category, "hits": 0, "total_expected": 0, "images_with_labels": 0, "false_positives": 0, "false_negatives": 0, "duplicate_predictions": 0, "timeouts": 0, "core": [], "wall": []})
            result["total_expected"] += len(expected)
            result["images_with_labels"] += 1
            result["core"].append(core)
            result["wall"].append(wall)
            if timed_out:
                result["timeouts"] += 1
                result["false_negatives"] += len(expected)
                continue
            hits, duplicates = score_localizations(expected, predicted)
            result["hits"] += hits
            result["false_negatives"] += len(expected) - hits
            result["false_positives"] += len(predicted) - hits
            result["duplicate_predictions"] += duplicates
        if seen_ids != set(truth_by_id):
            raise ValueError("prediction stream did not cover every truth image")
        category_rows = []
        for category in sorted(categories.values(), key=lambda item: item["name"]):
            expected_count = category["total_expected"]
            category_rows.append({
                "name": category["name"], "description": "WP-005 normalized prediction stream",
                "hits": category["hits"], "total_expected": expected_count,
                "images_with_labels": category["images_with_labels"],
                "rate_percent": 100.0 * category["hits"] / expected_count if expected_count else 0.0,
                "false_positives": category["false_positives"], "false_negatives": category["false_negatives"],
                "duplicate_predictions": category["duplicate_predictions"], "timeouts": category["timeouts"],
                "timeout_rate": category["timeouts"] / category["images_with_labels"],
                "core_runtime": runtime(category["core"]), "end_to_end_runtime": runtime(category["wall"]),
            })
        total_hits = sum(row["hits"] for row in category_rows)
        total_expected = sum(row["total_expected"] for row in category_rows)
        total_fp = sum(row["false_positives"] for row in category_rows)
        total_fn = sum(row["false_negatives"] for row in category_rows)
        total_images = sum(row["images_with_labels"] for row in category_rows)
        output = {
            "schema_version": OUTPUT_SCHEMA,
            "metadata": {
                "dataset_fingerprint": require_string(metadata, "dataset_fingerprint", "predictions.metadata"),
                "label_fingerprint": require_string(truth_metadata, "label_fingerprint", "truth.metadata"),
                "evaluator_fingerprint": EVALUATOR,
                "preprocessing_fingerprint": require_string(metadata, "preprocessing_fingerprint", "predictions.metadata"),
                "limit_per_category": metadata.get("limit_per_category"), "smoke": False,
                "selected_category": None, "commit_sha": require_string(metadata, "commit_sha", "predictions.metadata"),
                "normalization_source_schema": PREDICTION_SCHEMA,
            },
            "summary": {
                "weighted_global_rate_percent": 100.0 * total_hits / total_expected if total_expected else 0.0,
                "total_hits": total_hits, "total_expected": total_expected, "total_images_with_labels": total_images,
                "false_positives": total_fp, "false_negatives": total_fn,
                "duplicate_predictions": sum(row["duplicate_predictions"] for row in category_rows),
                "timeouts": sum(row["timeouts"] for row in category_rows),
                "timeout_rate": sum(row["timeouts"] for row in category_rows) / total_images if total_images else 0.0,
                "core_runtime": runtime(core_times), "end_to_end_runtime": runtime(wall_times),
            },
            "categories": category_rows,
        }
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(output, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(f"Normalized WP-005 v2 artifact: {args.output}")
        return 0
    except ValueError as error:
        print(f"ERROR: {error}", file=__import__("sys").stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())

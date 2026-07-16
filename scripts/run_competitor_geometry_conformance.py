#!/usr/bin/env python3
"""Verify synthetic shared-PGM payload and quadrilateral agreement.

This is a deliberately narrow protocol: six generated, axis-aligned Model 2
fixtures are materialized once as PGM and compared against their generator
geometry.  It is not real-scene localization recall, and process timing here
is not a fair latency boundary.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import subprocess
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
HARNESS_PATH = ROOT / "scripts/run_competitor_harness.py"
SPEC = importlib.util.spec_from_file_location("competitor_harness", HARNESS_PATH)
assert SPEC and SPEC.loader
HARNESS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(HARNESS)

SCHEMA = "rustqr.competitor-geometry-conformance.v1"
CASES = (
    "v01-H-m0-byte", "v01-M-m3-numeric", "v01-M-m3-alphanumeric",
    "v02-M-m0-byte", "v07-Q-m0-byte", "v20-L-m0-byte",
)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def canonicalize(corners: list[tuple[float, float]]) -> list[tuple[float, float]]:
    """Return a clockwise polygon whose first corner is top-left.

    The runner formats and libraries disagree on winding and start vertex; the
    canonical form avoids treating equivalent quadrilaterals as distinct.
    """
    if len(corners) != 4:
        raise ValueError(f"expected four corners, got {len(corners)}")
    area2 = sum(x1 * y2 - y1 * x2 for (x1, y1), (x2, y2)
                in zip(corners, corners[1:] + corners[:1]))
    ordered = list(reversed(corners)) if area2 < 0 else list(corners)
    start = min(range(4), key=lambda index: (ordered[index][1], ordered[index][0]))
    return ordered[start:] + ordered[:start]


def polygon_area(polygon: list[tuple[float, float]]) -> float:
    return abs(sum(x1 * y2 - y1 * x2 for (x1, y1), (x2, y2)
                   in zip(polygon, polygon[1:] + polygon[:1]))) / 2.0


def inside(point: tuple[float, float], edge_a: tuple[float, float], edge_b: tuple[float, float]) -> bool:
    return ((edge_b[0] - edge_a[0]) * (point[1] - edge_a[1])
            - (edge_b[1] - edge_a[1]) * (point[0] - edge_a[0])) >= -1e-9


def intersection(start: tuple[float, float], end: tuple[float, float],
                 edge_a: tuple[float, float], edge_b: tuple[float, float]) -> tuple[float, float]:
    sx, sy = start
    ex, ey = end
    ax, ay = edge_a
    bx, by = edge_b
    denominator = (ex - sx) * (by - ay) - (ey - sy) * (bx - ax)
    if abs(denominator) < 1e-12:
        return end
    factor = ((ax - sx) * (by - ay) - (ay - sy) * (bx - ax)) / denominator
    return sx + factor * (ex - sx), sy + factor * (ey - sy)


def intersection_polygon(subject: list[tuple[float, float]], clip: list[tuple[float, float]]) -> list[tuple[float, float]]:
    output = subject
    for edge_a, edge_b in zip(clip, clip[1:] + clip[:1]):
        input_points, output = output, []
        if not input_points:
            break
        previous = input_points[-1]
        for current in input_points:
            if inside(current, edge_a, edge_b):
                if not inside(previous, edge_a, edge_b):
                    output.append(intersection(previous, current, edge_a, edge_b))
                output.append(current)
            elif inside(previous, edge_a, edge_b):
                output.append(intersection(previous, current, edge_a, edge_b))
            previous = current
    return output


def iou(left: list[tuple[float, float]], right: list[tuple[float, float]]) -> float:
    intersection_area = polygon_area(intersection_polygon(left, right))
    union = polygon_area(left) + polygon_area(right) - intersection_area
    return intersection_area / union if union > 0 else 0.0


def parse_protocol(stdout: bytes) -> list[dict[str, Any]]:
    rows = []
    for line in stdout.splitlines():
        parts = line.split(b"\t")
        if len(parts) != 3 or parts[0] != b"G":
            raise ValueError(f"malformed geometry adapter row: {line!r}")
        points = []
        for item in parts[2].decode("ascii").split(";"):
            x, y = item.split(",", 1)
            points.append((float(x), float(y)))
        rows.append({"payload_hex": parts[1].decode("ascii"), "corners": canonicalize(points)})
    return rows


def parse_zbar_polygon(stdout: bytes) -> list[dict[str, Any]]:
    rows = []
    for line in stdout.splitlines():
        try:
            polygon, payload = line.rsplit(b":", 1)
            corners = [tuple(map(float, token.lstrip(b"+").split(b",", 1)))
                       for token in polygon.split()]
        except (ValueError, TypeError) as error:
            raise ValueError(f"malformed zbar --polygon row: {line!r}") from error
        rows.append({"payload_hex": payload.hex(), "corners": canonicalize(corners)})
    return rows


def command(adapter: str, pgm: Path) -> list[str]:
    binaries = ROOT / "competitors/bin"
    if adapter == "zbar":
        return ["zbarimg", "--quiet", "--raw", "--polygon", str(pgm)]
    if adapter == "quircs":
        return [str(binaries / "quircs_decode"), "--geometry", str(pgm)]
    if adapter == "rustqr":
        return [str(binaries / "rustqr_pgm_decode"), "--geometry", str(pgm)]
    raise ValueError(adapter)


def expected_corners(case: dict[str, Any]) -> list[tuple[float, float]]:
    matrix = case["matrix"]
    origin = matrix["quiet_zone_modules"] * matrix["pixels_per_module"]
    side = matrix["symbol_dimension"] * matrix["pixels_per_module"]
    return canonicalize([(origin, origin), (origin + side, origin),
                         (origin + side, origin + side), (origin, origin + side)])


def selected_cases(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    by_id = {case["id"]: case for case in manifest["cases"]}
    return [by_id[identifier] for identifier in CASES]


def run(manifest_path: Path, lock_path: Path) -> dict[str, Any]:
    manifest_bytes = manifest_path.read_bytes()
    manifest = json.loads(manifest_bytes)
    lock = json.loads(lock_path.read_text(encoding="utf-8"))
    runners = {name: HARNESS.adapter_preflight(name, lock["adapters"][name])
               for name in ("rustqr", "zbar", "quircs")}
    if any(result["status"] != "verified_pinned_runner" for result in runners.values()):
        raise RuntimeError("geometry conformance requires verified SHA-pinned runners")
    results = []
    with tempfile.TemporaryDirectory(prefix="rustqr-geometry-conformance-") as temporary:
        folder = Path(temporary)
        for index, case in enumerate(selected_cases(manifest)):
            source = manifest_path.parent / case["matrix"]["path"]
            if sha256(source) != case["matrix"]["sha256"]:
                raise ValueError(f"invalid fixture identity: {case['id']}")
            pgm = folder / f"{index:02d}.pgm"
            width, height = HARNESS.materialize_pgm(source, pgm, 1024)
            expected_payload = case["expected"]["raw_payload_hex"]
            expected = expected_corners(case)
            adapters = {}
            for name in runners:
                output = subprocess.run(command(name, pgm), capture_output=True, check=False, timeout=1)
                rows = parse_zbar_polygon(output.stdout) if name == "zbar" else parse_protocol(output.stdout)
                matches = [row for row in rows if row["payload_hex"] == expected_payload]
                geometry_iou = max((iou(expected, row["corners"]) for row in matches), default=0.0)
                adapters[name] = {
                    "returncode": output.returncode,
                    "raw_stdout_hex": output.stdout.hex(),
                    "raw_stderr_hex": output.stderr.hex(),
                    "records": [{**row, "corners": [[x, y] for x, y in row["corners"]]} for row in rows],
                    "exact_payload": len(matches) == 1,
                    "best_iou": geometry_iou,
                    "passes": output.returncode == 0 and len(matches) == 1 and geometry_iou >= 0.99,
                }
            results.append({
                "id": case["id"], "source_sha256": sha256(source), "pixel_sha256": sha256(pgm),
                "pixel_dimensions": [width, height], "expected_payload_hex": expected_payload,
                "expected_corners": [[x, y] for x, y in expected], "adapters": adapters,
            })
    passed = all(adapter["passes"] for row in results for adapter in row["adapters"].values())
    return {
        "schema_version": SCHEMA,
        "scope": "synthetic shared-PGM exact payload and quadrilateral conformance; not real-scene localization recall or fair latency",
        "iou_threshold": 0.99,
        "manifest": {"path": str(manifest_path), "sha256": hashlib.sha256(manifest_bytes).hexdigest()},
        "lock": {"path": str(lock_path), "sha256": sha256(lock_path)}, "runners": runners,
        "case_ids": list(CASES), "passed": passed, "cases": results,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=Path("conformance/manifest.json"))
    parser.add_argument("--lock", type=Path, default=Path("competitors/lock.json"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    report = run(args.manifest, args.lock)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())

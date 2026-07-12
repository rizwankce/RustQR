#!/usr/bin/env python3
"""Collect a repeatable WP-014 performance baseline without tuning the detector.

The probe intentionally keeps two timing boundaries separate:

* The Rust bench records in-process first-call, warm-call, allocation, and
  throughput measurements around `rust_qr::detect` only.
* This script records process-isolated `qrtool detect` wall time and sampled
  RSS, which includes image loading and CLI startup.

The real BoofCV ``lots`` candidate is not currently a successful decode.  The
result therefore records it as a negative multi-code control, not evidence for
the successful multi-code performance target in WP-014.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import re
import subprocess
import sys
import threading
import time
from pathlib import Path
from statistics import median
from typing import Any


CASES = {
    "clean": "benches/images/boofcv/nominal/image005.jpg",
    "hard": "benches/images/boofcv/damaged/image002.jpg",
    "multi_candidate": "benches/images/boofcv/lots/image001.jpg",
}
PROFILE_PREFIX = "WP014_PROFILE "


def percentile(samples: list[float], fraction: float) -> float:
    if not samples:
        return 0.0
    ordered = sorted(samples)
    position = (len(ordered) - 1) * fraction
    lower = int(position)
    upper = min(lower + 1, len(ordered) - 1)
    return ordered[lower] + (ordered[upper] - ordered[lower]) * (position - lower)


def git_value(repo: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args], cwd=repo, text=True, capture_output=True, check=False
    )
    return completed.stdout.strip() if completed.returncode == 0 else "unknown"


def sampled_rss_kb(process: subprocess.Popen[str], interval_s: float = 0.005) -> int | None:
    """Return a best-effort process RSS peak using portable `ps` polling.

    This is a sampled peak, so it must not be interpreted as a precise allocator
    profile.  The allocation probe is the authoritative allocation measurement.
    """

    maximum = 0
    while process.poll() is None:
        try:
            observed = subprocess.run(
                ["ps", "-o", "rss=", "-p", str(process.pid)],
                text=True,
                capture_output=True,
                check=False,
            ).stdout.strip()
        except OSError:
            # Sandboxed CI and restricted desktop environments can forbid
            # process inspection. Keep the latency/allocation artifact valid
            # and represent RSS as unavailable instead of emitting thread
            # errors or inventing a value.
            return None
        if observed.isdigit():
            maximum = max(maximum, int(observed))
        time.sleep(interval_s)
    return maximum or None


def run_process_case(qrtool: Path, image: Path) -> dict[str, Any]:
    command = [str(qrtool), "detect", "--image", str(image)]
    started = time.perf_counter_ns()
    process = subprocess.Popen(command, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    rss: list[int | None] = [None]
    sampler = threading.Thread(target=lambda: rss.__setitem__(0, sampled_rss_kb(process)))
    sampler.start()
    stdout, stderr = process.communicate()
    sampler.join()
    elapsed_ns = time.perf_counter_ns() - started
    found = re.search(r"Found (\d+) QR codes", stdout)
    return {
        "command": command,
        "elapsed_ns": elapsed_ns,
        "decoded_symbols": int(found.group(1)) if found else None,
        "sampled_peak_rss_kb": rss[0],
        "exit_code": process.returncode,
        "stderr": stderr.strip() or None,
    }


def run_allocation_probe(repo: Path, iterations: int) -> tuple[list[dict[str, Any]], str]:
    environment = os.environ.copy()
    environment["WP014_ALLOCATION_ONLY"] = "1"
    environment["WP014_PROFILE_ITERATIONS"] = str(iterations)
    command = [
        "cargo",
        "bench",
        "--bench",
        "wp014_profiles",
        "--features",
        "tools",
        "--",
        "--noplot",
    ]
    completed = subprocess.run(command, cwd=repo, text=True, capture_output=True, env=environment)
    output = completed.stdout + completed.stderr
    if completed.returncode:
        raise RuntimeError(f"allocation probe failed ({completed.returncode}):\n{output}")
    records = [
        json.loads(line[len(PROFILE_PREFIX) :])
        for line in output.splitlines()
        if line.startswith(PROFILE_PREFIX)
    ]
    if len(records) != len(CASES):
        raise RuntimeError(f"expected {len(CASES)} allocation records, got {len(records)}:\n{output}")
    return records, " ".join(command)


def summarize_process_runs(runs: list[dict[str, Any]]) -> dict[str, Any]:
    elapsed = [run["elapsed_ns"] for run in runs]
    decoded = [run["decoded_symbols"] for run in runs]
    rss = [run["sampled_peak_rss_kb"] for run in runs if run["sampled_peak_rss_kb"] is not None]
    median_ns = median(elapsed)
    return {
        "runs": runs,
        "median_ns": median_ns,
        "p95_ns": percentile(elapsed, 0.95),
        "images_per_second_from_median": 1_000_000_000 / median_ns if median_ns else 0.0,
        "decoded_symbols_per_run": decoded,
        "sampled_peak_rss_kb_max": max(rss) if rss else None,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="JSON baseline artifact path")
    parser.add_argument("--process-runs", type=int, default=5, help="fresh-process runs per lane")
    parser.add_argument("--in-process-iterations", type=int, default=5, help="detect calls per warm allocation lane")
    parser.add_argument("--skip-build", action="store_true", help="reuse target/release/qrtool")
    args = parser.parse_args()
    if args.process_runs < 1 or args.in_process_iterations < 1:
        parser.error("run counts must be positive")

    repo = Path(__file__).resolve().parents[1]
    if not args.skip_build:
        subprocess.run(
            ["cargo", "build", "--release", "--features", "tools", "--bin", "qrtool"],
            cwd=repo,
            check=True,
        )
    qrtool = repo / "target/release/qrtool"
    if not qrtool.is_file():
        parser.error(f"missing {qrtool}; omit --skip-build or build it first")

    allocation_records, allocation_command = run_allocation_probe(repo, args.in_process_iterations)
    allocation_by_lane = {record["lane"]: record for record in allocation_records}
    lanes: dict[str, Any] = {}
    for lane, relative_image in CASES.items():
        image = repo / relative_image
        if not image.is_file():
            raise FileNotFoundError(image)
        process_runs = [run_process_case(qrtool, image) for _ in range(args.process_runs)]
        probe = allocation_by_lane[lane]
        lanes[lane] = {
            "image": relative_image,
            "successful_first_call": probe["first_call_decoded"] > 0,
            "successful_warm_calls": probe["warm_decoded_total"] == probe["warm_iterations"],
            "in_process_detect_only": probe,
            "fresh_process_end_to_end": summarize_process_runs(process_runs),
        }

    artifact = {
        "schema_version": "rustqr.wp014.performance-baseline.v1",
        "generated_at_unix_ms": time.time_ns() // 1_000_000,
        "commit_sha": git_value(repo, "rev-parse", "HEAD"),
        "dirty_worktree": bool(git_value(repo, "status", "--porcelain")),
        "host": {
            "platform": platform.platform(),
            "python": sys.version.split()[0],
            "cpu_count": os.cpu_count(),
        },
        "methodology": {
            "allocation_probe_command": allocation_command,
            "in_process_boundary": "loaded RGB bytes to rust_qr::detect return",
            "process_boundary": "qrtool startup, image load, detect, and formatting",
            "rss": "best-effort sampled process RSS in KiB; not an allocation metric",
            "multi_lane": "negative candidate until WP-012 supplies a successful dense-scene fixture",
        },
        "lanes": lanes,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(artifact, indent=2, sort_keys=True) + "\n")
    print(f"WP-014 baseline: {args.output}")
    for lane, result in lanes.items():
        status = "success" if result["successful_warm_calls"] else "NOT-SUCCESSFUL"
        median_ms = result["in_process_detect_only"]["warm_per_call_ns"] / 1_000_000
        print(f"  {lane}: {status}; warm detect median-equivalent={median_ms:.3f} ms")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

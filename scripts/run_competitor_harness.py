#!/usr/bin/env python3
"""Run pinned OSS QR decoders against a shared, materialized pixel contract.

This is deliberately separate from RustQR's reading-rate evaluator. It produces
raw per-adapter observations and annotation-count coverage; it does not invent
payload or localization correctness where a competitor CLI cannot expose it.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import random
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

SCHEMA = "rustqr.competitor-report.v1"
CASE_SCHEMA = "rustqr.competitor-case-manifest.v1"
ADAPTERS = ("zxing_cpp", "quirc", "zbar", "boofcv", "opencv", "rqrr", "quircs", "rustqr")
REPOSITORY_ROOT = Path(__file__).resolve().parents[1]

# Keep process based adapters on the same one-thread contract as the OpenCV
# child process.  Some libraries ignore one or more of these knobs, but setting
# the common OpenMP/BLAS variables is both observable in the artifact and avoids
# silently granting a locally configured adapter extra worker threads.
THREAD_ENVIRONMENT = {
    "OMP_NUM_THREADS": "1",
    "OPENBLAS_NUM_THREADS": "1",
    "MKL_NUM_THREADS": "1",
    "VECLIB_MAXIMUM_THREADS": "1",
    "NUMEXPR_NUM_THREADS": "1",
}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def command_version(command: list[str]) -> str | None:
    try:
        result = subprocess.run(command, capture_output=True, timeout=10, check=False)
    except (OSError, subprocess.TimeoutExpired):
        return None
    output = (result.stdout + result.stderr).decode("utf-8", "replace").strip()
    return output.splitlines()[0] if output else None


def command_output(command: list[str]) -> str | None:
    """Read bounded local capability text without invoking an adapter on pixels."""
    try:
        result = subprocess.run(command, capture_output=True, timeout=10, check=False)
    except (OSError, subprocess.TimeoutExpired):
        return None
    return (result.stdout + result.stderr).decode("utf-8", "replace")


def hardware_metadata() -> dict[str, Any]:
    metadata: dict[str, Any] = {
        "platform": platform.platform(),
        "machine": platform.machine(),
        "processor": platform.processor(),
        "python": sys.version.split()[0],
        "cpu_count": os.cpu_count(),
    }
    for command, key in ((["sysctl", "-n", "machdep.cpu.brand_string"], "cpu_model"),):
        value = command_version(command)
        if value and "Operation not permitted" not in value:
            metadata[key] = value
    return metadata


def expected_symbols(label_path: Path) -> int:
    if not label_path.is_file():
        return 0
    points = [line for line in label_path.read_text("utf-8", errors="replace").splitlines()
              if line.strip() and not line.lstrip().startswith("#")]
    if len(points) % 4:
        raise ValueError(f"{label_path} has {len(points)} annotation points, not a multiple of four")
    return len(points) // 4


def collect_cases(dataset_root: Path, category: str | None, limit: int | None) -> list[dict[str, Any]]:
    categories = [category] if category else sorted(path.name for path in dataset_root.iterdir() if path.is_dir())
    cases: list[dict[str, Any]] = []
    for name in categories:
        folder = dataset_root / name
        if not folder.is_dir():
            raise ValueError(f"unknown dataset category: {name}")
        images = sorted([*folder.glob("*.jpg"), *folder.glob("*.jpeg"), *folder.glob("*.png")])
        if limit is not None:
            images = images[:limit]
        for image in images:
            label = image.with_suffix(".txt")
            cases.append({
                "id": f"{name}/{image.name}",
                "category": name,
                "source_path": str(image),
                "source_sha256": sha256(image),
                "expected_symbols": expected_symbols(label),
            })
    if not cases:
        raise ValueError("no images selected")
    return cases


def materialize_pgm(source: Path, destination: Path, max_dim: int) -> tuple[int, int]:
    try:
        from PIL import Image
    except ImportError as error:
        raise RuntimeError("Pillow is required to materialize the shared PGM pixels") from error
    with Image.open(source) as decoded:
        rgb = decoded.convert("RGB")
        if max(rgb.size) > max_dim:
            scale = max_dim / max(rgb.size)
            size = (max(1, round(rgb.width * scale)), max(1, round(rgb.height * scale)))
            # Pillow names the Triangle kernel BILINEAR.
            rgb = rgb.resize(size, Image.Resampling.BILINEAR)
        # Explicit BT.601 conversion avoids a decoder-specific image-loader path.
        gray = rgb.convert("L")
        gray.save(destination, format="PPM")
        return gray.size


def adapter_command(adapter: str, pgm: Path) -> tuple[list[str] | None, str | None]:
    binary = REPOSITORY_ROOT / "competitors/bin"
    commands: dict[str, list[str]] = {
        "zxing_cpp": [str(binary / "ZXingReader"), "-formats", "QRCode", str(pgm)],
        "quirc": [str(binary / "quirc_decode"), str(pgm)],
        "zbar": ["zbarimg", "--quiet", "--raw", str(pgm)],
        "boofcv": ["java", "-jar", str(binary / "boofcv_decode.jar"), str(pgm)],
        "rqrr": [str(binary / "rqrr_decode"), str(pgm)],
        "quircs": [str(binary / "quircs_decode"), str(pgm)],
        "rustqr": [str(binary / "rustqr_pgm_decode"), str(pgm)],
    }
    if adapter == "opencv":
        try:
            import cv2  # type: ignore
        except ImportError:
            return None, "cv2 is not installed"
        return [sys.executable, __file__, "--opencv-one", str(pgm)], None
    command = commands[adapter]
    if adapter == "boofcv" and not Path(command[2]).is_file():
        return None, f"required executable is unavailable: {command[2]}"
    executable = command[0]
    if "/" not in executable:
        available = shutil.which(executable) is not None
    else:
        available = Path(executable).is_file()
    return (command, None) if available else (None, f"required executable is unavailable: {executable}")


def adapter_setup(adapter: str) -> dict[str, Any]:
    """Describe the local runnable artifact without treating it as pinned."""
    command, unavailable = adapter_command(adapter, Path("<shared-pixels>.pgm"))
    if command is None:
        return {"status": "missing", "detail": unavailable}
    return {
        "status": "available",
        "command": command[:-1] + ["<shared-pixels>.pgm"],
    }


def adapter_runner_path(adapter: str, command: list[str]) -> Path | None:
    """Return the file whose digest identifies a file-based adapter runner."""
    if adapter == "boofcv":
        return Path(command[2])
    executable = command[0]
    if "/" in executable:
        return Path(executable).resolve()
    resolved = shutil.which(executable)
    return Path(resolved).resolve() if resolved else None


def adapter_preflight(adapter: str, record: dict[str, Any]) -> dict[str, Any]:
    """Record local runnable capability without asserting build provenance."""
    setup = adapter_setup(adapter)
    observed = observed_version(adapter)
    expected_version = record["version"]
    expected = expected_version.removeprefix("v")
    version_matches = observed is not None and expected in observed
    capabilities: dict[str, Any] = {}
    if adapter == "opencv" and setup["status"] == "available":
        try:
            import cv2  # type: ignore
            capabilities = {
                "qrcode_detector": hasattr(cv2, "QRCodeDetector"),
                "detect_and_decode_multi": hasattr(cv2.QRCodeDetector(), "detectAndDecodeMulti"),
            }
        except (ImportError, AttributeError):
            capabilities = {"qrcode_detector": False, "detect_and_decode_multi": False}
    elif adapter == "zbar" and setup["status"] == "available":
        help_text = command_output(["zbarimg", "--help"])
        capabilities = {"raw_payload_lines": help_text is not None and "--raw" in help_text}
    command, _ = adapter_command(adapter, Path("<shared-pixels>.pgm"))
    runner = adapter_runner_path(adapter, command) if command else None
    verified, verification_detail = verify_runner_provenance(
        runner, record.get("runner_sha256"), version_matches
    )
    if setup["status"] != "available":
        status = "missing_runner"
    elif not version_matches:
        status = "version_mismatch"
    elif verified:
        status = "verified_pinned_runner"
    else:
        # A system package/wheel can report the right version while differing
        # in build flags or origin.  Only a pinned build record can promote it
        # to a comparison-ready adapter.
        status = "available_version_matches_unverified_provenance"
    return {
        "status": status,
        "expected_version": expected_version,
        "version_observed": observed,
        "setup": setup,
        "capabilities": capabilities,
        "provenance_verified": verified,
        "provenance_verification_detail": verification_detail,
    }


def verify_runner_provenance(
    runner: Path | None, expected_sha256: str | None, version_matches: bool
) -> tuple[bool, str | None]:
    """Verify an explicitly locked local runner without inferring its origin."""
    if not version_matches:
        return False, "version does not match the lock"
    if not expected_sha256:
        return False, "lock has no runner_sha256"
    if runner is None or not runner.is_file():
        return False, "runner file is unavailable for hashing"
    observed = sha256(runner)
    if observed != expected_sha256:
        return False, "runner_sha256 mismatch"
    return True, None


def preflight(lock_path: Path, adapters: list[str]) -> dict[str, Any]:
    lock = json.loads(lock_path.read_text("utf-8"))
    if lock.get("schema_version") != "rustqr.competitor-lock.v1":
        raise ValueError("unexpected competitor lock schema")
    return {
        "schema_version": "rustqr.competitor-preflight.v1",
        "lock": {"path": str(lock_path), "sha256": sha256(lock_path)},
        "thread_environment": THREAD_ENVIRONMENT,
        "adapters": {
            adapter: adapter_preflight(adapter, lock["adapters"][adapter])
            for adapter in adapters
        },
        "comparison_claim": "none; availability and version are not pinned-build provenance",
    }


def decode_opencv_one(path: Path) -> int:
    import cv2  # type: ignore
    pixels = cv2.imread(str(path), cv2.IMREAD_GRAYSCALE)
    if pixels is None:
        return 2
    decoded = cv2.QRCodeDetector().detectAndDecodeMulti(pixels)
    if len(decoded) == 4 and decoded[0]:
        for item in decoded[1]:
            print(item)
        return 0
    single = cv2.QRCodeDetector().detectAndDecode(pixels)[0]
    if single:
        print(single)
        return 0
    return 1


def run_adapter(adapter: str, pgm: Path, timeout_ms: int) -> dict[str, Any]:
    command, unavailable = adapter_command(adapter, pgm)
    if command is None:
        return {"status": "unavailable", "detail": unavailable, "payloads_hex": [], "metadata": [],
                "metadata_status": "adapter_did_not_run", "raw_stdout_hex": None, "raw_stderr_hex": None,
                "latency_ms": None}
    try:
        started = time.perf_counter_ns()
        result = subprocess.run(
            command,
            capture_output=True,
            timeout=timeout_ms / 1000,
            check=False,
            env={**os.environ, **THREAD_ENVIRONMENT},
        )
        elapsed = (time.perf_counter_ns() - started) / 1_000_000
    except subprocess.TimeoutExpired:
        return {"status": "timeout", "detail": f"exceeded {timeout_ms} ms", "payloads_hex": [], "metadata": [],
                "metadata_status": "adapter_did_not_return_metadata", "raw_stdout_hex": None,
                "raw_stderr_hex": None, "latency_ms": timeout_ms}
    stdout = result.stdout.rstrip(b"\n")
    payloads = stdout.split(b"\n") if stdout else []
    status = "decoded" if result.returncode == 0 and payloads else "no_decode"
    return {
        "status": status,
        "detail": result.stderr.decode("utf-8", "replace").strip() or None,
        "payloads_hex": [payload.hex() for payload in payloads],
        # The adapters use a deliberately minimal payload-lines protocol. Keep
        # exact bytes and make lack of version/ECI/geometry metadata explicit.
        "metadata": [],
        "metadata_status": "adapter_protocol_does_not_expose_metadata",
        "raw_stdout_hex": result.stdout.hex(),
        "raw_stderr_hex": result.stderr.hex(),
        "latency_ms": elapsed,
    }


def percentile(samples: list[float], fraction: float) -> float | None:
    if not samples:
        return None
    ordered = sorted(samples)
    return ordered[min(len(ordered) - 1, int((len(ordered) - 1) * fraction + 0.999999))]


def bootstrap_ci(samples: list[float], seed: int = 0xC0DEC0DE, rounds: int = 1000) -> list[float | None]:
    if not samples:
        return [None, None]
    rng = random.Random(seed)
    medians = [statistics.median(rng.choices(samples, k=len(samples))) for _ in range(rounds)]
    return [percentile(medians, 0.025), percentile(medians, 0.975)]


def summarize(rows: list[dict[str, Any]]) -> dict[str, Any]:
    # Only calls that actually crossed an adapter invocation boundary may
    # contribute to timing or annotation-count coverage. In particular, a
    # missing, version-mismatched, or provenance-unverified runner must not
    # look like a zero-return decode in a report.
    completed = [
        row for row in rows
        if row["result"]["status"] in {"decoded", "no_decode", "timeout"}
    ]
    latencies = [row["result"]["latency_ms"] for row in completed if row["result"]["latency_ms"] is not None]
    expected = sum(row["expected_symbols"] for row in completed)
    returned = sum(len(row["result"]["payloads_hex"]) for row in completed)
    return {
        "cases": len(rows),
        "available_cases": len(completed),
        "expected_annotation_symbols": expected,
        "returned_symbols": returned,
        "annotation_count_coverage": (returned / expected) if expected else None,
        "status_counts": {status: sum(row["result"]["status"] == status for row in rows)
                          for status in sorted({row["result"]["status"] for row in rows})},
        "latency_ms": {
            "samples": len(latencies), "median": statistics.median(latencies) if latencies else None,
            "p95": percentile(latencies, 0.95), "median_95_ci": bootstrap_ci(latencies),
        },
    }


def run(args: argparse.Namespace) -> dict[str, Any]:
    lock_path = Path(args.lock)
    lock = json.loads(lock_path.read_text("utf-8"))
    if lock.get("schema_version") != "rustqr.competitor-lock.v1":
        raise ValueError("unexpected competitor lock schema")
    adapters = args.adapter or list(ADAPTERS)
    cases = collect_cases(Path(args.dataset_root), args.category, args.limit)
    observed_versions = {adapter: observed_version(adapter) for adapter in adapters}
    setup = {adapter: adapter_setup(adapter) for adapter in adapters}
    runner_preflight = {
        adapter: adapter_preflight(adapter, lock["adapters"][adapter])
        for adapter in adapters
    }
    results: dict[str, list[dict[str, Any]]] = {adapter: [] for adapter in adapters}
    with tempfile.TemporaryDirectory(prefix="rustqr-competitor-pixels-") as temporary:
        pixel_root = Path(temporary)
        materialized = []
        for index, case in enumerate(cases):
            pgm = pixel_root / f"{index:05d}.pgm"
            width, height = materialize_pgm(Path(case["source_path"]), pgm, args.max_dim)
            materialized.append({**case, "pixel_sha256": sha256(pgm), "pixel_dimensions": [width, height], "pgm": pgm})
        case_manifest = {
            "schema_version": CASE_SCHEMA,
            "lock_sha256": sha256(lock_path),
            "pixel_contract": lock["pixel_contract"],
            "max_dim": args.max_dim,
            "cases": [{key: value for key, value in case.items() if key != "pgm"} for case in materialized],
        }
        for adapter in adapters:
            expected_version = lock["adapters"][adapter]["version"].removeprefix("v")
            observed = observed_versions[adapter]
            version_mismatch = observed is not None and expected_version not in observed
            runner_unverified = (
                args.require_verified_runner
                and runner_preflight[adapter]["status"] != "verified_pinned_runner"
            )
            for case in materialized:
                result = (
                    {"status": "version_mismatch", "detail": f"pinned {expected_version}, observed {observed}",
                     "payloads_hex": [], "metadata": [], "metadata_status": "adapter_did_not_run",
                     "raw_stdout_hex": None, "raw_stderr_hex": None, "latency_ms": None}
                    if version_mismatch else run_adapter(adapter, case["pgm"], args.timeout_ms)
                )
                if runner_unverified:
                    result = {
                        "status": "unverified_runner",
                        "detail": (
                            "--require-verified-runner requires a matching version and "
                            "runner_sha256; "
                            f"preflight status is {runner_preflight[adapter]['status']}"
                        ),
                        "payloads_hex": [], "metadata": [],
                        "metadata_status": "adapter_did_not_run",
                        "raw_stdout_hex": None, "raw_stderr_hex": None,
                        "latency_ms": None,
                    }
                results[adapter].append({
                    "id": case["id"], "category": case["category"],
                    "expected_symbols": case["expected_symbols"],
                    "pixel_sha256": case["pixel_sha256"],
                    "result": result,
                })
    return {
        "schema_version": SCHEMA,
        "lock": {"path": str(lock_path), "sha256": sha256(lock_path)},
        "case_manifest": case_manifest,
        "hardware": hardware_metadata(),
        "thread_environment": THREAD_ENVIRONMENT,
        "adapters": {adapter: {"lock": lock["adapters"][adapter], "setup": setup[adapter],
                                "runner_preflight": runner_preflight[adapter],
                                "version_observed": observed_versions[adapter],
                                "summary": summarize(rows), "cases": rows}
                     for adapter, rows in results.items()},
    }


def observed_version(adapter: str) -> str | None:
    binary = REPOSITORY_ROOT / "competitors/bin"
    if adapter == "boofcv" and not (binary / "boofcv_decode.jar").is_file():
        return None
    commands = {"zbar": ["zbarimg", "--version"], "zxing_cpp": [str(binary / "ZXingReader"), "--version"],
                "quirc": [str(binary / "quirc_decode"), "--version"], "boofcv": ["java", "-jar", str(binary / "boofcv_decode.jar"), "--version"],
                "rqrr": [str(binary / "rqrr_decode"), "--version"],
                "quircs": [str(binary / "quircs_decode"), "--version"],
                "rustqr": [str(binary / "rustqr_pgm_decode"), "--version"],
                "opencv": [sys.executable, "-c", "import cv2; print(cv2.__version__)"]}
    return command_version(commands[adapter])


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dataset-root", default="benches/images/boofcv", type=Path)
    parser.add_argument("--lock", default="competitors/lock.json", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--category")
    parser.add_argument("--limit", type=int)
    parser.add_argument("--max-dim", type=int, default=1024)
    parser.add_argument("--timeout-ms", type=int, default=1000)
    parser.add_argument("--adapter", action="append", choices=ADAPTERS)
    parser.add_argument("--require-verified-runner", action="store_true",
                        help="do not invoke adapters unless version and runner digest match the lock")
    parser.add_argument("--preflight", action="store_true", help="write local adapter availability only; do not select images or decode")
    parser.add_argument("--opencv-one", type=Path, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.opencv_one:
        return decode_opencv_one(args.opencv_one)
    if not args.output:
        parser.error("--output is required")
    try:
        report = preflight(Path(args.lock), args.adapter or list(ADAPTERS)) if args.preflight else run(args)
    except (OSError, RuntimeError, ValueError, json.JSONDecodeError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

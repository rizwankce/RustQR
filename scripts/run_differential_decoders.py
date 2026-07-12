#!/usr/bin/env python3
"""Run reproducible, offline differential QR decoders over a conformance manifest."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
from collections import Counter
from pathlib import Path
from typing import Callable

SCHEMA = "rustqr.differential-report.v1"
MANIFEST_SCHEMA = "rustqr.conformance-manifest.v1"


def command_version(command: list[str]) -> str:
    try:
        result = subprocess.run(command, capture_output=True, check=False, timeout=10)
    except (OSError, subprocess.TimeoutExpired):
        return "unknown"
    text = (result.stdout + result.stderr).decode("utf-8", "replace").strip()
    return text.splitlines()[0] if text else "unknown"


def decode_zbar(image: Path) -> tuple[str, bytes | None, str | None]:
    executable = shutil.which("zbarimg")
    if executable is None:
        return "unavailable", None, "zbarimg not found on PATH"
    try:
        result = subprocess.run(
            [executable, "--quiet", "--raw", str(image)],
            capture_output=True,
            check=False,
            timeout=30,
        )
    except subprocess.TimeoutExpired:
        return "timeout", None, "decoder exceeded 30 seconds"
    if result.returncode != 0:
        return "decode_error", None, result.stderr.decode("utf-8", "replace").strip()
    # zbarimg appends one record separator newline in --raw mode.
    payload = result.stdout[:-1] if result.stdout.endswith(b"\n") else result.stdout
    return "decoded", payload, None


def decode_opencv(image: Path) -> tuple[str, bytes | None, str | None]:
    try:
        import cv2  # type: ignore
    except ImportError:
        return "unavailable", None, "Python module cv2 is not installed"
    pixels = cv2.imread(str(image))
    if pixels is None:
        return "decode_error", None, "image could not be loaded"
    try:
        decoded = cv2.QRCodeDetector().detectAndDecode(pixels)[0]
    except cv2.error as error:
        return "decode_error", None, str(error)
    if not decoded:
        return "decode_error", None, "no payload decoded"
    # OpenCV exposes text, not arbitrary raw codeword bytes. UTF-8 fixtures are
    # comparable; binary fixtures are reported as unsupported by the caller.
    return "decoded", decoded.encode("utf-8"), None


ADAPTERS: dict[str, Callable[[Path], tuple[str, bytes | None, str | None]]] = {
    "zbar": decode_zbar,
    "opencv": decode_opencv,
}


def adapter_metadata(name: str) -> dict:
    if name == "zbar":
        executable = shutil.which("zbarimg")
        return {
            "available": executable is not None,
            "engine": "ZBar",
            "version": command_version([executable, "--version"]) if executable else None,
            "raw_bytes": True,
        }
    try:
        import cv2  # type: ignore

        version = cv2.__version__
        available = True
    except ImportError:
        version = None
        available = False
    return {
        "available": available,
        "engine": "OpenCV QRCodeDetector",
        "version": version,
        "raw_bytes": False,
    }


def run(manifest_path: Path, adapter_names: list[str]) -> dict:
    manifest_bytes = manifest_path.read_bytes()
    manifest = json.loads(manifest_bytes)
    if manifest.get("schema_version") != MANIFEST_SCHEMA:
        raise ValueError(f"expected {MANIFEST_SCHEMA} manifest")
    root = manifest_path.parent
    rows = []
    corpus_counts: Counter[str] = Counter()
    for case in manifest.get("cases", []):
        expected_hex = case.get("expected", {}).get("raw_payload_hex")
        expected_outcome = case.get("expected", {}).get("outcome", "decode_success")
        if expected_outcome not in ("decode_success", "reject"):
            raise ValueError(f"invalid expected.outcome for {case.get('id')}: {expected_outcome}")
        matrix_path = case.get("matrix", {}).get("path")
        matrix_status = case.get("matrix", {}).get("status") or (
            "generated" if matrix_path else "planned"
        )
        corpus_counts[matrix_status] += 1
        result_row = {
            "id": case.get("id"),
            "expected": case.get("expected", {}),
            "matrix_status": matrix_status,
            "decoders": {},
        }
        for name in adapter_names:
            if matrix_status == "unsupported_mode":
                status, payload, detail = (
                    "unsupported",
                    None,
                    f"fixture mode {case.get('mode')} is explicitly unsupported",
                )
            elif not matrix_path:
                status, payload, detail = "not_generated", None, "matrix.path is absent"
            elif matrix_status != "generated":
                status, payload, detail = (
                    "not_generated",
                    None,
                    f"matrix status is {matrix_status}",
                )
            elif not (root / matrix_path).is_file():
                status, payload, detail = "invalid_fixture", None, "matrix file is absent"
            elif case.get("matrix", {}).get("sha256") and case["matrix"][
                "sha256"
            ] != hashlib.sha256((root / matrix_path).read_bytes()).hexdigest():
                status, payload, detail = "invalid_fixture", None, "matrix SHA-256 mismatch"
            elif name == "opencv" and expected_hex:
                expected = bytes.fromhex(expected_hex)
                try:
                    expected.decode("utf-8")
                except UnicodeDecodeError:
                    status, payload, detail = (
                        "unsupported",
                        None,
                        f"{name} adapter exposes text and cannot verify arbitrary raw bytes",
                    )
                else:
                    status, payload, detail = ADAPTERS[name](root / matrix_path)
            else:
                status, payload, detail = ADAPTERS[name](root / matrix_path)
            if status == "decoded":
                if expected_outcome == "reject":
                    status = "unexpected_accept"
                else:
                    status = "match" if payload.hex() == expected_hex else "mismatch"
            elif status == "decode_error" and expected_outcome == "reject":
                status = "expected_rejection"
            result_row["decoders"][name] = {
                "status": status,
                "raw_payload_hex": payload.hex() if payload is not None else None,
                "detail": detail,
            }
        rows.append(result_row)
    summaries = {}
    for name in adapter_names:
        counts = Counter(row["decoders"][name]["status"] for row in rows)
        summaries[name] = {
            "status_counts": dict(sorted(counts.items())),
            "generated_total": corpus_counts["generated"],
            "matches": counts["match"],
            "mismatches": counts["mismatch"] + counts["unexpected_accept"],
        }
    return {
        "schema_version": SCHEMA,
        "manifest": {
            "path": str(manifest_path),
            "sha256": hashlib.sha256(manifest_bytes).hexdigest(),
            "schema_version": MANIFEST_SCHEMA,
        },
        "adapters": {name: adapter_metadata(name) for name in adapter_names},
        "corpus": {
            "case_count": len(rows),
            "matrix_status_counts": dict(sorted(corpus_counts.items())),
        },
        "summary": summaries,
        "cases": rows,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=Path("conformance/manifest.json"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--adapter", action="append", choices=sorted(ADAPTERS))
    args = parser.parse_args()
    adapters = args.adapter or sorted(ADAPTERS)
    try:
        report = run(args.manifest, adapters)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Compare verified payload-only competitors on a small conformance subset.

This is deliberately not a localization or end-to-end latency comparison:
every adapter exposes payload lines only. It supplies exact payload truth for
the RustQR, ZBar, and quircs shared-PGM runners without inventing geometry.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import tempfile
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
HARNESS_PATH = ROOT / "scripts/run_competitor_harness.py"
SPEC = importlib.util.spec_from_file_location("competitor_harness", HARNESS_PATH)
assert SPEC and SPEC.loader
HARNESS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(HARNESS)

SCHEMA = "rustqr.competitor-payload-conformance.v1"
# ASCII-only normal-mode cases keep the shared newline protocol byte-exact.
DEFAULT_CASES = (
    "v01-H-m0-byte",
    "v01-M-m3-numeric",
    "v01-M-m3-alphanumeric",
    "v02-M-m0-byte",
    "v07-Q-m0-byte",
    "v20-L-m0-byte",
)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def select_cases(manifest: dict, selected: tuple[str, ...]) -> list[dict]:
    by_id = {case["id"]: case for case in manifest.get("cases", [])}
    rows = []
    for identifier in selected:
        case = by_id.get(identifier)
        if case is None:
            raise ValueError(f"selected case is absent: {identifier}")
        expected = case.get("expected", {})
        matrix = case.get("matrix", {})
        payload = expected.get("raw_payload_hex")
        if (expected.get("outcome", "decode_success") != "decode_success"
                or matrix.get("status") != "generated" or not payload):
            raise ValueError(f"selected case lacks generated payload truth: {identifier}")
        try:
            bytes.fromhex(payload).decode("ascii")
        except UnicodeError as error:
            raise ValueError(f"selected case is not ASCII-safe: {identifier}") from error
        rows.append(case)
    return rows


def classify(result: dict, expected: str) -> dict:
    payloads = result["payloads_hex"]
    if result["status"] != "decoded":
        return {**result, "payload_status": "no_payload"}
    if payloads == [expected]:
        return {**result, "payload_status": "exact_match"}
    return {**result, "payload_status": "mismatch"}


def run(manifest_path: Path, lock_path: Path, selected: tuple[str, ...]) -> dict:
    manifest_bytes = manifest_path.read_bytes()
    manifest = json.loads(manifest_bytes)
    if manifest.get("schema_version") != "rustqr.conformance-manifest.v1":
        raise ValueError("unexpected conformance manifest schema")
    lock = json.loads(lock_path.read_text(encoding="utf-8"))
    runners = {name: HARNESS.adapter_preflight(name, lock["adapters"][name])
               for name in ("rustqr", "zbar", "quircs")}
    if any(row["status"] != "verified_pinned_runner" for row in runners.values()):
        raise RuntimeError("payload conformance requires all verified shared-PGM runners")
    cases = select_cases(manifest, selected)
    rows = []
    with tempfile.TemporaryDirectory(prefix="rustqr-competitor-conformance-") as temporary:
        temp = Path(temporary)
        for index, case in enumerate(cases):
            source = manifest_path.parent / case["matrix"]["path"]
            if not source.is_file() or sha256(source) != case["matrix"]["sha256"]:
                raise ValueError(f"invalid fixture identity: {case['id']}")
            pgm = temp / f"{index:02d}.pgm"
            width, height = HARNESS.materialize_pgm(source, pgm, 1024)
            expected = case["expected"]["raw_payload_hex"]
            results = {name: classify(HARNESS.run_adapter(name, pgm, 1000), expected)
                       for name in runners}
            rows.append({
                "id": case["id"], "mode": case["mode"],
                "source_sha256": sha256(source), "pixel_sha256": sha256(pgm),
                "pixel_dimensions": [width, height], "expected_payload_hex": expected,
                "adapters": results,
            })
    summary = {name: dict(sorted(Counter(row["adapters"][name]["payload_status"]
                                          for row in rows).items())) for name in runners}
    return {
        "schema_version": SCHEMA,
        "scope": "verified shared-PGM payload-only conformance; not localization or end-to-end latency comparison",
        "manifest": {"path": str(manifest_path), "sha256": hashlib.sha256(manifest_bytes).hexdigest()},
        "lock": {"path": str(lock_path), "sha256": sha256(lock_path)},
        "runners": runners, "selected_case_ids": list(selected), "summary": summary, "cases": rows,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=Path("conformance/manifest.json"))
    parser.add_argument("--lock", type=Path, default=Path("competitors/lock.json"))
    parser.add_argument("--case", action="append", choices=DEFAULT_CASES)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    report = run(args.manifest, args.lock, tuple(args.case or DEFAULT_CASES))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

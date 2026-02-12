# RustQR

RustQR is an in-progress QR decoder rebuild focused on measurable reading-rate gains on the BoofCV benchmark set.

## Project Status

- Rebuild branch is active; architecture and benchmark harness are implemented and evolving.
- Current goal: push BoofCV reading rate toward `>=90%` with median scan time `<=1s`.
- Main source-of-truth plan: `docs/new_from_scratch.md`.

## Quick Start

Requirements:
- Rust stable toolchain (`rustup` + `cargo`)

Build and test:

```bash
cargo build
cargo test
```

Run smoke command:

```bash
cargo run --bin qrtool -- smoke
```

## Reading-Rate Benchmark

Run a benchmark and write an artifact:

```bash
cargo run --bin qrtool -- reading-rate --profile boofcv-all --artifact target/reading_rate_boofcv_all.json
```

Run strict payload-validation lane:

```bash
cargo run --bin qrtool -- reading-rate --profile payload-validated --artifact target/reading_rate_payload_validated.json
```

Important label semantics:
- `boofcv-*` profiles use annotation labels, so match means "decode found" (not strict payload equality).
- `payload-validated` uses expected payload labels with exact payload matching.

Available profile families:
- `boofcv-all`
- `boofcv-<category>` where category is one of: `blurred`, `brightness`, `bright-spots`, `close`, `curved`, `damaged`, `glare`, `high-version`, `lots`, `monitor`, `nominal`, `noncompliant`, `pathological`, `perspective`, `rotations`, `shadows`
- `payload-validated`

Legacy aliases kept for compatibility:
- `monitor-smoke` -> `boofcv-monitor`
- `nominal-smoke` -> `boofcv-nominal`

## GitHub Actions

- `CI`: build + test on push/PR.
- `Benchmark`: manual workflow dispatch with configurable inputs.
  - Use `profile=all-profiles` to run every benchmark lane in one run.
  - Artifacts are uploaded for offline comparison and tracking.

## BoofCV Reading Rate Baseline (Reference Table)

Latest RustQR values below are from GitHub Actions benchmark run `21927167412` on `scratch_from_scratch_rebuild` (completed on `2026-02-12`).
The BoofCV lane is annotation-based ("any decode" counts as match).

| Category | Images | Dynamsoft | BoofCV | ZBar | RustQR |
|----------|--------|-----------|--------|------|--------|
| blurred | 45 | 66.15% | 38.46% | 35.38% | 31.11% |
| brightness | 28 | 81.18% | 78.82% | 50.59% | 25.00% |
| bright_spots | 32 | 43.30% | 27.84% | 19.59% | 0.00% |
| close | 40 | 95.00% | 100.00% | 12.50% | 97.50% |
| curved | 50 | 70.00% | 56.67% | 35.00% | 26.00% |
| damaged | 37 | 51.16% | 16.28% | 25.58% | 21.62% |
| glare | 50 | 84.91% | 32.08% | 35.85% | 20.00% |
| high_version | 33 | 97.30% | 40.54% | 27.03% | 9.09% |
| lots | 7 | 100.00% | 99.76% | 18.10% | 0.00% |
| monitor | 17 | 100.00% | 82.35% | 0.00% | 94.12% |
| nominal | 65 | 93.59% | 89.74% | 66.67% | 73.85% |
| noncompliant | 16 | 92.31% | 3.85% | 50.00% | 6.25% |
| pathological | 23 | 95.65% | 43.48% | 65.22% | 56.52% |
| perspective | 35 | 62.86% | 80.00% | 42.86% | 74.29% |
| rotations | 44 | 99.25% | 96.24% | 48.87% | 45.45% |
| shadows | 14 | 100.00% | 85.00% | 90.00% | 35.71% |
| total | 536 | 83.29% | 60.69% | 38.95% | 41.60% |

## Documentation

- Strategy and architecture: `docs/new_from_scratch.md`
- Lessons from the old code path: `docs/learnings_from_old_code_base_try.md`
- Active work packets: `docs/todo/01_feature_todo.txt`
- Completed work packets: `docs/completed/01_feature.todo.txt`

## Contributing

Issues and pull requests are welcome.

Before opening a PR, run:

```bash
cargo fmt -- --check
cargo test
```

For benchmark-impacting changes, attach benchmark artifact(s) and summarize deltas.

## License

Licensed under either:
- MIT license
- Apache License, Version 2.0

See `Cargo.toml` (`MIT OR Apache-2.0`).

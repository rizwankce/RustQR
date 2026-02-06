# Repository Guidelines

## Project Structure & Module Organization
- `src/lib.rs`: public API (`detect`, `Detector`) and pipeline orchestration.
- `src/detector/`: finder, alignment, timing, transform, and pyramid detection logic.
- `src/decoder/`: format/version parsing, Reed-Solomon, mode decoding, and bitstream handling.
- `src/models/` and `src/utils/`: core data types and low-level image/math helpers.
- `src/bin/qrtool.rs` and `src/tools/`: CLI + benchmark helpers (enabled with `tools` feature).
- `tests/`: integration/regression tests using real images.
- `benches/`: Criterion benchmarks; datasets live under `benches/images/`.
- `docs/`: roadmap, optimization notes, spec references, and changelog.

## Build, Test, and Development Commands
- `cargo build`: build the library.
- `cargo test`: run unit + integration tests.
- `cargo test --lib --release`: match CI’s release-library test pass.
- `cargo fmt -- --check`: enforce formatting (CI-gated).
- `cargo clippy --all-targets --all-features`: lint checks before opening PRs.
- `cargo bench -- qr_detect`: run a focused synthetic benchmark.
- `cargo bench --features tools --bench real_qr_images`: run real-image benchmark.
- `cargo run --features tools --bin qrtool -- reading-rate --limit 3`: quick reading-rate smoke run.

## Coding Style & Naming Conventions
- Rust edition: 2024; format with `rustfmt` defaults (4-space indentation).
- Use `snake_case` for functions/modules/files, `PascalCase` for structs/enums, `SCREAMING_SNAKE_CASE` for constants.
- Keep modules focused by stage (`detector`, `decoder`, `utils`) and prefer small, testable functions.
- Avoid broad `allow` attributes; add narrowly scoped exceptions with a short reason.

## Testing Guidelines
- Place integration regressions in `tests/*_tests.rs`; name tests by scenario (for example, `test_decode_rotated`).
- Use deterministic assertions on decoded content/metadata when possible.
- Real-image tests may be tuned with env vars:
  - `QR_MAX_DIM=1024` is the default recommendation for benchmark/CI parity.
  - `QR_MAX_DIM=800` for faster local iteration.
  - `QR_MAX_DIM=1200` for occasional deep validation.
  - `QR_MAX_DIM=0` disables downscaling.
  - `QR_DEBUG=1` enables debug logs.
- Run targeted tests during iteration, then `cargo test` before commit.

## Commit & Pull Request Guidelines
- Recent history favors concise, imperative subjects; optional prefixes are common: `feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`.
- Keep commits scoped (one logical change each) and include benchmark/test updates when behavior changes.
- PRs should include:
  - clear summary and motivation,
  - linked issue/task (if available),
  - commands run locally (test/lint/bench),
  - before/after metrics for performance-sensitive changes.

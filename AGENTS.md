# Repository Guidelines

## Project Structure & Module Organization
`src/lib.rs` is the public library entry point and exports the decode API. Core pipeline stages live in `src/pipeline/` (`hypothesis_search`, `geometry_refinement`, `decode_engine`, `multi_qr_iteration`, and shared `state`). CLI entry points are in `src/bin/`, with `qrtool` exposing `smoke` and `reading-rate` commands. Benchmark and dataset tooling is centralized in `src/tools/mod.rs`.  
Integration tests are in `tests/` and are organized by work packet (`wp002_...`, `wp010_...`). Benchmark datasets and labels are in `benches/images/boofcv` and `benches/images/custom/decoding`. Design notes and active/completed work packets are in `docs/`.

## Build, Test, and Development Commands
- `cargo build --locked`: build with dependency lockfile parity (matches CI).
- `cargo test --locked`: run full integration/unit test suite.
- `cargo fmt -- --check`: enforce formatting before PR.
- `cargo run --bin qrtool -- smoke`: quick local scaffold sanity check.
- `cargo run --bin qrtool -- reading-rate --profile boofcv-all --artifact target/reading_rate_boofcv_all.json`: run benchmark lane and write JSON artifact.
- `cargo test --test wp010_decode_backend_tests`: run one integration suite while iterating.

## Coding Style & Naming Conventions
Use Rust 2024 idioms and keep code `rustfmt`-clean (4-space indentation, default style). File/module/function names are `snake_case`; structs/enums/traits are `CamelCase`; constants are `SCREAMING_SNAKE_CASE`. Preserve deterministic, budget-aware behavior in pipeline code and avoid introducing nondeterministic ordering. Unsafe code is disallowed (`#![forbid(unsafe_code)]`).

## Testing Guidelines
Add integration coverage in `tests/` for any behavior change, especially pipeline bounds, determinism, and benchmark argument parsing. Prefer descriptive test names that state behavior (for example, `decode_candidate_count_is_globally_bounded`). Keep fixtures and dataset assumptions explicit in the test body. For benchmark-impacting changes, generate an artifact under `target/` and summarize KPI deltas in the PR.

## Commit & Pull Request Guidelines
Follow the existing Conventional Commit style seen in history: `feat:`, `fix:`, `perf:`, `bench:`, `docs:`, `ci:`, `chore:`. Keep subjects imperative and scoped to one change. PRs should include: what changed, why, commands run (`cargo fmt -- --check`, `cargo test --locked`), and benchmark artifacts/delta notes when reading-rate behavior is affected.

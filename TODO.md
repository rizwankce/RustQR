# RustQR Roadmap

This file is the execution queue for future agent sessions. Work through the
packets in order unless a packet explicitly says it can run in parallel.

When starting a new session, ask the agent to:

1. Read `AGENTS.md`, this file, and the Markdown files referenced by the chosen
   work packet.
2. Inspect the current branch and working tree before editing.
3. Work on one packet only unless the packet explicitly authorizes broader
   changes.
4. Run targeted local checks during development.
5. Use GitHub Actions for full benchmark runs; do not run the complete image
   corpus locally by default.
6. Record completed commands, results, decisions, and follow-up work in this
   file before handing the task back.

## Operating principles

- Correctness and safety come before performance claims.
- A benchmark improvement is valid only when its scoring and timing boundaries
  are trustworthy.
- Keep the deterministic specification path separate from bounded recovery
  heuristics.
- Do not tune against every benchmark image. Preserve validation and holdout
  data to detect overfitting.
- Do not merge the rebuild branch wholesale until it has been compared against
  `main` using the same evaluator and input pixels.
- Preserve unrelated user changes and the untracked `benchmark/` artifacts.
- Correct public claims when evidence contradicts them.

## Benchmark ladder

Use the smallest useful run while iterating.

### Targeted local run

```bash
QR_MAX_DIM=800 cargo run --features tools --bin qrtool --release -- \
  reading-rate --category rotations --limit 3
```

Increase `--limit` to 5, 10, or 25 only when the smaller run is stable.

### Multiple categories locally

Run categories separately so the output identifies regressions clearly:

```bash
QR_MAX_DIM=800 cargo run --features tools --bin qrtool --release -- \
  reading-rate --category rotations --limit 5

QR_MAX_DIM=800 cargo run --features tools --bin qrtool --release -- \
  reading-rate --category high_version --limit 5
```

### Environment overrides

- `QR_BENCH_LIMIT=N`: maximum images per category.
- `QR_BENCH_LIMIT=0`: full dataset.
- `QR_SMOKE=1`: use the smoke subset.
- `QR_MAX_DIM=800`: fast local iteration.
- `QR_MAX_DIM=1024`: GitHub Actions and comparison default.
- `QR_MAX_DIM=1200`: occasional deeper validation.
- `QR_MAX_DIM=0`: preserve original resolution.

The CLI `--limit` value takes precedence over `QR_BENCH_LIMIT`. When neither is
set, the current implementation runs the full dataset.

### GitHub Actions

- `Fast Benchmark`: defaults to macOS and 25 images per category; supports a
  category filter and selected platform.
- `Full Benchmark`: defaults to all 536 images on Linux, macOS, and Windows,
  with `QR_MAX_DIM=1024`.
- `Criterion Benchmark`: use its smoke option during iteration; reserve full
  runs for performance milestones.

Run the full workflow only for milestone candidates, architecture comparisons,
or release baselines.

## Required checks for ordinary code changes

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Real-image tests are currently ignored by default. Run the relevant ignored
test explicitly when a packet affects that scenario.

---

## Continuation handoff (2026-07-12)

The current dirty working tree is the active implementation. **Do not require
it to be committed or pushed before continuing local work.** Preserve all
existing changes and the untracked `benchmark/` evidence. WP-002's remaining
Fast Benchmark run is a remote validation gate only; it blocks WP-005 and
WP-013 comparisons, but it does **not** block the independent WP-006 work below.

### Start here

Run these quick checks before editing:

```bash
git status --short
cargo test --test conformance_matrix_tests --all-features
cargo test --test conformance_mutation_tests --all-features
python3 -m unittest discover -s scripts/tests -p 'test_*.py'
```

Then choose exactly one of these resumable packets. They may run in parallel
when each worker stays within the listed ownership boundary.

#### WP-006A: Full supported Model 2 grid

**Status:** Completed locally on 2026-07-12

**Ownership:** `conformance/`, `scripts/generate_conformance_manifest.py`,
`scripts/materialize_conformance_matrices.py`, generator/materializer tests,
and a new generated-corpus integration test. Avoid decoder implementation files.

- Generate on demand for versions 1-40, all four EC levels, all eight masks,
  and supported numeric/alphanumeric/byte modes.
- Keep large generated matrices out of Git unless a compact representative set
  is needed; the full gate should regenerate into a temporary directory.
- Assert exact raw payload, version, EC level, and mask for every valid case.
- Record deterministic seeds, backend version, and checksums.

**Done when:** the full supported grid regenerates reproducibly and RustQR
passes 100%; failures remain visible rather than being removed from the grid.

#### WP-006B: Unsupported mode implementation

**Status:** Completed locally (compact end-to-end fixtures)

**Ownership:** decoder mode/payload/model files plus focused unit fixtures.
Avoid corpus generator and benchmark evaluator files.

- Implement one capability at a time in this order: Kanji raw bytes and count,
  ECI metadata, GS1/FNC1, then Structured Append metadata.
- Preserve raw payload bytes; do not force unsupported encodings through UTF-8.
- Replace `unsupported_mode` manifest entries only after exact payload and
  metadata tests pass.

**2026-07-12 WP-006B update:** Added a dedicated QR Kanji decoder that consumes
the version-dependent character count and converts each 13-bit QR value back
to its original Shift-JIS byte pair. `QRCode::data` now preserves those raw
bytes, and `decode_matrix_for_mode` advertises Kanji support; ECI, GS1/FNC1,
and Structured Append remain explicit unsupported modes. Deterministic focused
fixtures cover the two-character `漢字` payload (`8abf8e9a`) and truncated
Kanji data. Passed:
`cargo test --lib decoder::modes::kanji --all-features`,
`cargo test --lib test_decode_payload_kanji_preserves_shift_jis_bytes --all-features`,
`cargo test --lib matrix_api_reports_unsupported_modes_explicitly --all-features`,
`cargo test --test conformance_matrix_tests --all-features` (6 passed, one
full-grid test ignored), and `git diff --check`. The compact null-matrix Kanji
fixture is now an invalid-input case rather than an unsupported-mode claim.
The generated manifest remains `unsupported_mode` because the pinned
python-qrcode materializer cannot emit a Kanji matrix; add an independently
generated matrix and exact matrix metadata assertions before changing that
corpus status. This historical status was superseded by the 2026-07-12
completion record below.

**2026-07-12 WP-006B metadata update:** ECI, GS1/FNC1, and Structured Append
headers are now decoded into `QRCode::metadata`. ECI handles all ISO prefix
widths (8, 16, and 24 bits); FNC1 first/second position is recorded and applies
the required alphanumeric `%` substitution; Structured Append records
zero-based index, total symbol count, and parity. Focused bitstream fixtures
assert raw payload, rendered content, and exact metadata. The conformance
manifest entries remain `unsupported_mode` only because the pinned
python-qrcode materializer cannot create header-bearing matrices. Passed:
`cargo fmt -- --check`, `cargo test --lib --all-features` (100 passed),
`cargo test --test conformance_matrix_tests --all-features` (6 passed, one
full-grid test ignored), and `git diff --check`. Follow-up: materialize
independently generated ECI, GS1/FNC1, Structured Append, and Kanji matrices,
then change their manifest statuses and add end-to-end matrix metadata checks.
This historical status was superseded by the completion record below.

**Completion record (2026-07-12):** Materialized deterministic valid Kanji,
ECI, GS1/FNC1, and Structured Append matrices in the compact foundation
corpus. `scripts/materialize_conformance_matrices.py` now contains a narrow
ISO header-segment adapter that writes the otherwise unsupported mode/header
bits directly and delegates RS coding/module placement to pinned
`python-qrcode==8.2`. The matrix integration test verifies raw payload,
version, EC level, mask, and public metadata (ECI assignment 26, FNC1-first,
and Structured Append index 2 of 4 with parity `0xa7`). GS1 uses a real
alphanumeric `%` separator and asserts its decoded `0x1d` group separator.
The header fixtures are compact representatives rather than part of the 3,840
numeric/alphanumeric/byte full-grid gate. Passed:

```bash
python3 scripts/generate_conformance_manifest.py --output conformance/manifest.json
python3 scripts/materialize_conformance_matrices.py --manifest conformance/manifest.json
python3 -m unittest discover -s scripts/tests -p 'test_*.py'
cargo test --test conformance_matrix_tests --all-features
```

**Done when:** each implemented capability has deterministic valid fixtures and
explicit metadata assertions. Complete for the supported scope above.

#### WP-006C: Remaining block-layout mutations

**Status:** Completed locally on 2026-07-12; independent of WP-006A and WP-006B

**Ownership:** `scripts/qr_spec_mapping.py`, mutation planner/materializer,
`conformance/mutations*`, and `tests/conformance_mutation_tests.rs`. Avoid core
decoder changes unless a generated fixture exposes a proven decoder bug.

- Replace the seven remaining `planned` cross-block cases with provably
  over-limit deterministic mutations, or document why a case is impossible.
- Regenerate mutation provenance and verify byte-identical output.
- Require invalid layouts to be rejected; accepted garbage is a hard failure.

**Done when:** no block-layout mutation remains ambiguously `planned`.

**Completion record (2026-07-12):** Replaced the first-two-block random-prefix
swap selection with deterministic unequal-codeword matching across every RS
block pair. All 12 multi-block cases now have `correction_limit_per_block <
errors_per_affected_block` evidence and reject in the matrix decoder; 18
single-block cases remain explicitly `not_applicable`. Verified with:

```bash
python3 scripts/materialize_conformance_mutations.py
sha256sum conformance/mutations.json  # unchanged after a second materialization
python3 -m unittest discover -s scripts/tests -p 'test_*.py'
cargo test --test conformance_mutation_tests --all-features
```

### Remote-only follow-up

WP-002 Fast Benchmark must run only after the current local changes are
committed and pushed. If the session is not explicitly authorized to commit and
push, leave this gate pending and continue WP-006A, WP-006B, or WP-006C. Do not
stop all work merely because Actions cannot yet exercise the dirty worktree.

### Current verified baseline

- `cargo fmt -- --check`: passed.
- `cargo clippy --all-targets --all-features -- -D warnings`: passed.
- `cargo test --all-features`: 97 library, 5 CLI, 6 conformance matrix,
  4 mutation, 7 input API, 2 timeout, and 1 doc test passed; the full-grid
  conformance gate and 7 slow photographic tests are ignored by design.
- Supported compact corpus: RustQR 34/34. Independent decoder evidence is
  recorded per fixture in `conformance/manifest.json` and
  `conformance/differential-report.json`; adapter limitations (Kanji text
  transcoding and unsupported header modes) are explicit and there are zero
  raw-payload mismatches.

---

## WP-001: Preserve and document benchmark evidence

**Status:** Completed on 2026-07-11

**Goal:** Prevent the untracked GitHub benchmark artifacts from being lost and
establish their provenance without presenting them as an authoritative
baseline.

**Read first:**

- `README.md`
- `docs/reading_rate_improvement.md`
- `docs/failure_cluster_triage.md`
- `.github/workflows/benchmark.yml`
- `benchmark/baselines/gh_run_21927167412/*.json`

**Tasks:**

- Confirm which branch and commit produced GitHub run `21927167412`.
- Record the command, environment, platform, dataset fingerprint, evaluator
  schema, and timing boundaries.
- Determine whether the artifacts should be checked in, attached to a release,
  or retained as Actions artifacts with longer retention.
- Make backup label files and unrelated documentation irrelevant to the dataset
  fingerprint.
- Clearly label the BoofCV lane as annotation/count based and the custom lane as
  payload validated.

**Acceptance criteria:**

- Artifacts have documented provenance and cannot be mistaken for main's
  authoritative baseline.
- Dataset, evaluator, preprocessing, and artifact fingerprints are separately
  identifiable.
- No benchmark result is silently deleted or overwritten.

**Completion record:**

- Confirmed with `gh run view 21927167412` and the Actions log that the run was
  dispatched from `scratch_from_scratch_rebuild` at
  `e735ac991fb27e4c8b3c5c70d7e51c7fa3f61de6`, on GitHub's Ubuntu 24.04 runner
  image `20260201.15.1` with Rust 1.93.0.
- Inspected the workflow at that commit and recorded the expanded command,
  environment, preprocessing, evaluator semantics, and timing boundaries in
  `benchmark/baselines/gh_run_21927167412/README.md`.
- Generated independent dataset-image, evaluator, pipeline/preprocessing,
  dependency-lock, and preserved-artifact SHA-256 fingerprints. The dataset
  fingerprint uses image Git blobs only, so backup labels and unrelated docs do
  not affect it.
- Added `SHA256SUMS` without modifying any of the 17 preserved JSON artifacts.
  `shasum -a 256 -c SHA256SUMS` passes from the artifact directory.
- Decision: check in this 448 KiB historical evidence set because the original
  Actions artifact has expired. Keep future large artifacts in Actions or a
  release asset with explicit retention, and use immutable run-ID directories.
- Follow-up for WP-002: add separate label-input fingerprints to the next schema
  and reject comparisons against `wp007-reading-rate-v1` unless explicitly
  opted into historical compatibility.
- Known gap: the run log proves `reading_rate_boofcv-all.json` was generated and
  uploaded, but it is missing from the preserved copy and the expired Actions
  artifact can no longer be downloaded. The omission is documented; no result
  was reconstructed or overwritten.

---

## WP-002: Repair reading-rate scoring and timing

**Status:** Completed locally and validated in Fast Benchmark Actions

**Goal:** Make benchmark results trustworthy enough to compare branches and
competitors.

**Primary files:**

- `src/bin/qrtool.rs`
- `src/tools/mod.rs`
- `src/models/qr_code.rs`
- `.github/workflows/benchmark.yml`
- `scripts/compare_reading_rate_artifacts.py`

**Tasks:**

- Include every production fallback inside the measured latency interval.
- Report core pipeline and end-to-end latency separately.
- Stop treating `min(results, expected)` as sufficient correctness.
- Match returned QR quadrilaterals to BoofCV annotations one-to-one.
- Report localization precision, recall, F1, duplicate predictions, and false
  positives.
- Keep exact payload matching as a separate metric for datasets that contain
  payload ground truth.
- Add p50, p90, p95, p99, timeout rate, images/second, and QR symbols/second.
- Version the evaluator schema and reject incompatible baseline comparisons.
- Add evaluator unit tests for duplicates, missing symbols, extra symbols,
  invalid labels, multi-QR scenes, and timeouts.

**Acceptance criteria:**

- A wrong payload cannot count as a payload success.
- A prediction in the wrong location cannot count as a localization success.
- Extra predictions reduce precision.
- All work contributing to the result is included in end-to-end timing.
- Old and new artifact schemas cannot be compared accidentally.

**Validation:**

- Use synthetic evaluator fixtures locally.
- Use `--category nominal --limit 3` and `--category lots --limit 1` locally.
- Run Fast Benchmark in Actions after local tests pass.

**Progress record (2026-07-11):**

- Added strict synthetic label parsing and initial one-to-one localization
  scoring tests, plus separate core/end-to-end timing summaries and v2 schema
  rejection in the artifact comparator.
- Populated `QRCode.position` from the exact successful sampling transform on
  regular, inverted, jittered, high-version-refined, and fallback decode paths.
  A focused test verifies the projected outer symbol boundary and corner order.
- Added separate image-input and label-input fingerprints plus evaluator,
  preprocessing, category, limit, smoke, and evaluated-category compatibility
  checks. The comparator rejects old schemas and incompatible v2 scopes by
  default. Payload metrics are explicitly unavailable (`null`) for the BoofCV
  annotation-only dataset rather than inferred from detection counts.
- Added a caller-supplied cooperative detector deadline for benchmark runs.
  Decoder and pipeline loops observe the tighter of this deadline and the
  configured production budget; elapsed end-to-end time still determines the
  timeout metric and late results are discarded. This cannot preempt a single
  uninterruptible operation. A strict wall-clock kill would require process or
  worker isolation and belongs with WP-008 benchmark isolation, not a thread
  cancellation claim in WP-002.
- Replaced bounding-box/greedy matching with winding-safe convex quadrilateral
  overlap and maximum-cardinality bipartite matching. Labels are scaled into
  processed-image coordinates before scoring; load failures count as misses and
  contribute end-to-end timing samples.
- Added explicit `--payload-validated` mode for same-stem payload labels. It
  performs normalized, exact, one-to-one payload matching and cannot be selected
  implicitly by malformed localization labels.
- Targeted release validation passed: `nominal --limit 3` localized 3/6 symbols
  and `lots --limit 1` localized 0/60 symbols. A 1 ms timeout run reported one
  timeout, a 1.0 timeout rate, and accepted no late result.
- `cargo fmt -- --check`, strict all-target/all-feature Clippy, all-feature
  tests, three comparator tests, and `git diff --check` pass.
- Fast Benchmark Actions run `29199249805` completed successfully on
  `codex/wp006-conformance` (macOS, 25 images per category). It used the v2
  localization evaluator and reported 8.9231% weighted-global localization
  rate, 1.0 precision, 0.089231 recall, zero timeouts, and 901.00 ms median
  end-to-end image time. These are validation evidence, not a performance
  claim; the run's low recall is intentionally carried into WP-005/WP-014.

---

## WP-003: Add a safe, fallible public input API

**Status:** Completed (2026-07-11)

**Goal:** Prevent malformed dimensions or short buffers from reaching unchecked
SIMD and pointer operations.

**Primary files:**

- `src/lib.rs`
- `src/utils/grayscale.rs`
- `src/utils/memory_pool.rs`
- `src/models/qr_code.rs`

**Tasks:**

- Introduce checked multiplication for width, height, stride, and channel
  calculations.
- Validate input buffer length before conversion or detection.
- Introduce an explicit pixel format and optional stride.
- Add a fallible API returning structured errors.
- Decide whether the existing convenience API remains as a compatibility
  wrapper or is deprecated.
- Keep unsafe kernels private and document their proven preconditions.
- Add tests for empty input, short input, extra padding, zero dimensions,
  enormous dimensions, multiplication overflow, RGB, RGBA, and grayscale.
- Add fuzz targets for public image entry points.

**Acceptance criteria:**

- Safe public APIs cannot cause out-of-bounds pointer reads for arbitrary input.
- Invalid input returns a deterministic error rather than an empty detection or
  panic.
- Existing valid callers have a documented migration path.

**Results:**

- Added `try_detect(ImageInput)` with explicit grayscale, RGB, and RGBA pixel
  formats, optional row stride, and structured `InputError` failures.
- Kept `detect` and `detect_from_grayscale` as compatibility wrappers; invalid
  legacy input now returns an empty result, while new callers can migrate to
  the fallible API for deterministic error details.
- Added checked dimension/channel/stride calculations and last-addressed-byte
  validation. Row padding and trailing bytes are accepted.
- Hardened public grayscale conversion helpers against short buffers and
  arithmetic overflow, documented private unsafe-kernel preconditions, and
  removed unsafe buffer-length manipulation from `BufferPool`.
- Added `tests/input_api_tests.rs` and a cargo-fuzz target covering the public
  image input API.

**Validation:**

- `cargo test --lib --no-default-features` — passed (67 tests).
- `cargo test --test input_api_tests` — passed (7 tests).
- `cargo test --all-features` — passed (72 library tests, 7 input API tests,
  and 1 doc test; 7 slow real-image regressions remained ignored).
- `cargo fmt -- --check` — passed.
- `cargo clippy --all-targets --all-features -- -D warnings` — passed.
- Fuzz target compile was not checked because restricted DNS prevented fetching
  `libfuzzer-sys`; the harness is recorded under `fuzz/` for a networked run.

---

## WP-004: Establish truthful capabilities and documentation

**Status:** Completed on 2026-07-11

**Goal:** Make the public project description match tested behavior.

**Primary files:**

- `README.md`
- `Cargo.toml`
- `docs/spec.md`
- `docs/optimize.md`
- `docs/decoder_status.md`
- `docs/reading_rate_improvement.md`
- `CLAUDE.md`
- `benches/images/README.md`

**Tasks:**

- Create a capability matrix for Model 1, Model 2, Micro QR, versions, EC
  levels, masks, modes, ECI, GS1/FNC1, Structured Append, inversion, mirrored
  codes, multi-QR, platforms, and `no_std`.
- Label every capability as tested, partial, planned, or unsupported.
- Remove or qualify claims about zero dependencies, zero unsafe code, `no_std`,
  world-leading speed, platform support, Model 1, and Micro QR.
- Explain the difference between microbenchmarks and successful end-to-end
  decode latency.
- Fix stale documentation saying the default benchmark limit is five.
- Identify dated historical documents as snapshots rather than current truth.

**Acceptance criteria:**

- Every headline feature links to a test, CI lane, or explicit roadmap status.
- Local and Actions benchmark instructions agree with the code.
- Performance claims identify dataset, commit, platform, preprocessing,
  evaluator, and timing boundary.

**Completion record:**

- Replaced the README feature list with an evidence-qualified capability
  matrix covering models, versions, EC levels, masks, modes, ECI, Kanji,
  GS1/FNC1, Structured Append, inversion, orientation/mirroring, multi-QR,
  platforms, and `no_std`.
- Identified Model 1 and Micro QR as metadata placeholders rather than scanner
  support. Marked ECI, Kanji, inverted/oriented symbols, high versions, and
  multi-QR as partial where code paths exist without sufficient end-to-end
  fixtures.
- Removed the zero-dependency, zero-unsafe, `no_std`, unverified platform, and
  fastest-scanner package claims. Documented the checked WP-003 input API and
  the private validated SIMD `unsafe` boundary.
- Aligned README, decoder snapshot, and dataset instructions with the live
  full-dataset default, CLI-over-environment limit precedence, Fast Benchmark
  limit of 25, and Full Benchmark's 536-image desktop matrix at
  `QR_MAX_DIM=1024`.
- Distinguished component Criterion measurements from successful end-to-end
  decode latency. The old README table is now identified as historical run
  `21837108650` evidence whose count-based evaluator is not a current accuracy
  or competitor baseline.
- Marked `docs/optimize.md`, `docs/decoder_status.md`, and
  `docs/reading_rate_improvement.md` as dated snapshots/worklogs rather than
  current truth.

**Validation:**

- `cargo test --lib --all-features` — passed (77 tests).
- `cargo fmt -- --check` — could not provide an isolated result because
  concurrent WP-002 edits in `src/bin/qrtool.rs` were not yet formatted; WP-004
  changed Markdown and Cargo metadata only.
- Audited `.github/workflows/ci.yml`, `benchmark.yml`,
  `fast-benchmark.yml`, and `criterion-benchmark.yml` against the documented
  platform, limit, preprocessing, and smoke-run claims.

---

## WP-005: Compare main with the rebuild branch

**Status:** Direction decided; normalized cross-branch v2 benchmark remains
pending

**Goal:** Decide whether to continue `main`, adopt the rebuild, or merge proven
slices from both.

**Branches:**

- `main`
- `work_wp014_wp018_gates_2026_02_12`

**Tasks:**

- Compare public API, architectural complexity, code size, safety, standards
  coverage, tests, and maintainability.
- Run identical evaluator fixtures on both branches.
- Run targeted local categories first: `nominal`, `rotations`, `perspective`,
  `high_version`, `lots`, `brightness`, and `bright_spots`.
- Trigger the same Fast Benchmark configuration for both branches.
- Trigger Full Benchmark only after targeted results are understood.
- Produce a per-category accuracy and latency delta table.
- Identify individual rebuild commits or modules worth transplanting.
- Write an architecture decision record with the chosen direction.

**Acceptance criteria:**

- The decision is supported by the same pixels, evaluator, preprocessing, and
  hardware class.
- Improvements are not accepted when they hide category regressions or timeout
  work.
- There is a concrete merge or transplant sequence with rollback points.

**Progress record (2026-07-12):**

- The Fast Benchmark gate cited by WP-002 completed successfully, so a local
  comparison was run from isolated exports of `main` (`5b9b41e`), the actual
  rebuild branch `scratch_from_scratch_rebuild` (`295b97c`), and the current
  Model 2 branch (`4de7966`). The older packet's
  `work_wp014_wp018_gates_2026_02_12` branch label is not the rebuild branch
  associated with the historical benchmark evidence.
- All three binaries used the same current-worktree image pixels at an 800-px
  working limit over nominal, rotations, perspective, high_version, lots,
  brightness, and bright_spots. The combined selected-image-list SHA-256 was
  `37fdaff3033d604e9f616e671f1ab2e72adadd692f48eadeced56448163117ca`.
  The six ordinary categories used three images and lots used one.
- The figures cannot be made into accuracy deltas: main emits count-based
  `rustqr.reading_rate.v1`, current emits one-to-one localization
  `rustqr.reading_rate.v2`, and the rebuild emits per-image
  `wp007-reading-rate-v1` (where an annotation image succeeds on any decode).
  The v2 comparator correctly rejects main's v1 artifact. The diagnostic
  sample and architecture decision are recorded in
  `docs/adr/0001-wp005-branch-comparison.md`.
- Decision: retain the current Model 2 implementation; do not merge the
  rebuild or accept any direct transplant. Its proposal-ensemble/multi-QR
  commits are quarantined idea sources only until they pass the current
  evaluator and conformance gates. `cargo test --all-features --no-fail-fast`
  passed in all three isolated exports; only the current branch has the
  3,840-fixture conformance gate.
- Remaining evidence gap: normalize the rebuild's predictions through the v2
  localization evaluator (or a shared external v2 evaluator), then run the
  same targeted categories and Fast Benchmark configuration at 1024 px before
  accepting any slice. The ADR contains the slice-by-slice rollback sequence.

---

## WP-006: Build an ISO conformance and differential corpus

**Status:** Complete; full supported grid and compact header corpus are green

**Goal:** Prove decoder correctness independently of photographic detection.

**Tasks:**

- Generate Model 2 matrices across versions 1-40, all EC levels, all masks, and
  representative capacity boundaries.
- Cover numeric, alphanumeric, byte, Kanji, ECI, GS1/FNC1, and Structured
  Append.
- Include valid error and erasure patterns up to correction limits.
- Add invalid format, version, remainder, padding, and block-layout cases.
- Differentially compare generated fixtures against at least two independent
  mature decoders.
- Store generation seeds and expected raw payload bytes and metadata.

**Acceptance criteria:**

- The deterministic matrix decoder reaches 100% on supported valid fixtures.
- Unsupported features return explicit errors.
- Invalid matrices do not produce accepted garbage payloads.
- All fixtures are reproducible from a manifest and seed.

**Progress record (2026-07-11):**

- Added `rustqr.conformance-manifest.v1`, a deterministic stdlib-only generator,
  and a compact checked-in foundation manifest. Stable cases record seed,
  Model 2 version, EC level, mask, mode, expected raw payload bytes, expected
  metadata, and placeholders for independently generated matrix checksums and
  differential-decoder evidence.
- The compact profile covers every metadata axis without checking in the full
  Cartesian product; `--full` reproducibly expands all 40 versions, four EC
  levels, eight masks, and seven mode scaffolds.
- ZBar and OpenCV differential results are recorded for all 34 generated
  matrices and copied into each manifest case. ZBar has 32 exact byte matches,
  one Kanji text match (it transcodes Shift-JIS to UTF-8), and one decode
  failure for Structured Append. OpenCV has 27 exact matches, six decode
  failures, and one explicit unsupported raw-byte comparison for Kanji. Neither
  adapter has a raw-payload mismatch; the checked-in report records versions,
  fixture checksums, and every limitation rather than treating failures as
  successes.
- Materialized 34 supported numeric, alphanumeric, byte, Kanji, ECI,
  GS1/FNC1, and Structured Append matrices with pinned `python-qrcode==8.2`,
  fixed version/EC/mask inputs, reproducible PNG checksums, and explicit
  rendering metadata. Header fixtures use the documented ISO segment adapter.
- Binary-searched and recorded 22 version/EC/mode capacity maxima using the
  pinned backend; each record asserts maximum accepted and maximum-plus-one
  rejected. The materializer and all emitted checksums are deterministic across
  consecutive runs.
- Materialized correctable-error mutations for all 34 generated matrices; the
  deterministic matrix decoder exactly reproduces every expected payload.
  All 34 erasure variants are now executable through the request-scoped
  `decode_matrix_with_erasures` API using either row-major module confidence or
  explicit erased-module coordinates. Evidence is validated, mapped to
  per-block codeword erasures, and exact payloads pass at each fixture's full
  supported erasure boundary without global state. Focused conformance and
  Reed-Solomon boundary tests pass. All 12 multi-block invalid block-layout
  mutations are materialized and rejected; deterministic unequal-codeword
  matching records an error count above the correction limit in both affected
  blocks. The 18 single-block cases are correctly marked not applicable.
- Materialized deterministic invalid-format and invalid-version matrices by
  replacing both redundant information regions at their specification-defined
  coordinates. Their manifest records SHA-256 checksums and mutation evidence,
  and matrix-entry-point tests prove rejection without invoking photographic
  detection or recovery fallbacks.
- Materialized deterministic invalid-remainder and invalid-padding matrices
  from the explicit Model 2 placement map. The remainder fixture records the
  exact version-2 module coordinate; the padding fixture changes the first
  `0xEC` pad byte to `0xED`, regenerates Reed-Solomon parity with the pinned
  backend, and records all eight mapped module coordinates. Strict matrix
  decoding now rejects non-zero remainder modules and invalid terminator,
  alignment, or alternating padding bits without mistaking them for RS damage.
- Corrected stale corpus evidence labels: erasure mutations now record
  `materialized` and document their executable
  `decode_matrix_with_erasures` path, matching the existing 34-fixture test.
- Repository validation passed: formatting, strict all-target/all-feature
  Clippy, 94 library tests, five CLI tests, six conformance matrix tests, three
  mutation tests, seven input API tests, two timeout tests, and one doc test.
  The seven slow photographic regression tests remain intentionally ignored.
- The checked-in corpus remains compact, while the ignored full-grid gate
  regenerates versions 1-40 by four EC levels by eight masks for numeric,
  alphanumeric, and byte modes. Kanji, ECI, GS1/FNC1, and Structured Append
  have deterministic compact end-to-end matrices with exact metadata checks.
  RustQR passes all 34/34 compact cases; independent-adapter outcomes and
  limitations are recorded per case.
- WP-006A (2026-07-12): added an on-demand ignored integration gate that
  regenerates the full supported Model 2 grid in a temporary directory rather
  than checking in 3,840 PNGs. It verifies every generated numeric,
  alphanumeric, and byte fixture's recorded checksum, exact raw payload,
  Model 2 version, EC level, and mask. `cargo test --test
  conformance_matrix_tests --all-features -- --ignored
  generated_full_supported_corpus_reaches_one_hundred_percent` passed.
  `python3 -m unittest scripts.tests.test_generate_conformance_manifest
  scripts.tests.test_materialize_conformance_matrices` passed. The ordinary
  conformance-matrix run was rerun after the WP-006B fixture update and passes
  with six standard tests; the full-grid test remains intentionally ignored
  except when explicitly requested.
- **Completion audit (2026-07-13):** Re-ran the full ignored gate after fixing
  its full-profile-only capacity-probe defect: capacity probes stay in the
  foundation corpus (22 representative version/EC/mode records) instead of
  being redundantly binary-searched for all 3,840 full-grid symbols. This
  avoids a known `python-qrcode==8.2` overflow-path `glog(0)` failure without
  weakening either coverage set. `cargo test --test conformance_matrix_tests
  --all-features -- --ignored generated_full_supported_corpus_reaches_one_hundred_percent`
  passed (1 test, 131.77 s). The standard matrix suite passed (7 tests), the
  mutation suite passed (4 tests), and the generator/materializer/mutation/
  differential Python tests passed (16 tests). A canonical rerun of
  `scripts/run_differential_decoders.py` was byte-for-byte identical to
  `conformance/differential-report.json`: 34 generated compact fixtures,
  ZBar and OpenCV, and zero raw-payload mismatches. WP-006 acceptance is met:
  deterministic supported fixtures decode exactly, invalid fixtures reject,
  and the manifest/seed materializers reproduce the committed corpus.

---

## WP-007: Replace brute-force decoding with a spec-first decoder

**Status:** In progress; deterministic traversal/structural-validation slice complete

**Goal:** Make the common decode path deterministic, fast, and resistant to
false positives.

**Primary files:**

- `src/decoder/qr_decoder/matrix_decode.rs`
- `src/decoder/qr_decoder/payload.rs`
- `src/decoder/format.rs`
- `src/decoder/version.rs`
- `src/decoder/bitstream.rs`
- `src/decoder/function_mask.rs`

**Tasks:**

- Use the specification-defined traversal as the primary path.
- Correct full BCH handling for format and version information.
- Validate finder, timing, alignment, dark module, remainder, terminator, and
  padding invariants.
- Fix Kanji character count and Shift-JIS decoding.
- Preserve raw bytes and honor ECI rather than assuming UTF-8.
- Add GS1/FNC1 and Structured Append metadata.
- Move soft-format, mask, traversal, and module-repair searches into an explicit
  bounded recovery phase.
- Rank recovery attempts by evidence and stop at request deadlines.

**Acceptance criteria:**

- Clean valid matrices require one deterministic decoding path.
- Recovery cannot return a result that fails structural validation.
- Exact payload, segment, and metadata results match WP-006 fixtures.
- Common-case matrix decode latency drops substantially without recall loss.

**Progress record (2026-07-12):**

- Made the normal matrix-decoding path specification-first: it now attempts
  only the canonical bottom-right, upward, right-column-first data traversal
  with an exact BCH format candidate. Alternate traversal orderings, soft
  format candidates, brute-force EC/mask hypotheses, and uncertain-module
  repair are isolated in the bounded recovery phase.
- Added fixed-function validation for all three finder patterns, timing
  patterns, alignment patterns, and the dark module. The normal path requires
  an exact match; recovery permits at most three mismatches per pattern but
  never accepts a candidate that fails this validation.
- Replaced the unused even-parity `BchDecoder` placeholder with real masked
  BCH(15,5) nearest-codeword decoding over all 32 format codewords, bounded to
  the ISO correction radius of three bits. Tests cover the correction boundary
  and structural-pattern damage/tolerance boundary.
- Passed `cargo test --lib --all-features` (104 tests), `cargo test --test
  conformance_matrix_tests --all-features` (6 passed; full 3,840-fixture gate
  intentionally ignored), and `git diff --check`.
- Remaining WP-007 work: make version-information placement/validation share
  the same explicit BCH evidence path, measure common-case matrix latency
  against the pre-slice baseline, and verify recovery recall on the targeted
  photographic categories before claiming the packet complete.

**2026-07-12 version BCH update:** Consolidated QR format BCH(15,5) and
version BCH(18,6) construction and nearest-codeword decoding in
`src/decoder/bch.rs`, retaining the original `decode_format` API and exposing
distance evidence for validation paths. `VersionInfo::extract_with_evidence`
now decodes each redundant version-information copy independently and records
which copy and BCH distance won deterministically. A generated Version 7
matrix test proves that four damaged modules in one copy recover from the
pristine copy, while four damaged modules in both copies are rejected by the
strict `QrDecoder::decode_matrix` entry point. Passed:
`cargo test --lib --all-features` (106 passed) and
`cargo test --test conformance_matrix_tests
version_information_uses_independent_bch_protected_copies --all-features`.
The latency and photographic-recall acceptance work remains outstanding.

**2026-07-12 recovery ordering update:** Recovery now orders eligible
orientations by a deterministic fixed-pattern mismatch score, retaining source
order as the tie-breaker. Exact BCH format evidence is still tried before soft
format evidence (which remains BCH-distance ordered), and soft traversals,
brute-force EC/mask hypotheses, and beam repairs check the cooperative request
deadline before every individual attempt. The beam-repair budget also stops
when that request deadline expires. This does not relax structural acceptance:
all ranked candidates first pass the existing tolerant structural gate.

Targeted photographic regression validation at `QR_MAX_DIM=800` passed all
seven ignored cases: monitor, blurred, high-version, rotated, damaged,
multiple-code, and nominal. The known high-version/blurred/damaged/nominal
limitations still report their warnings, so this is a no-regression recovery
check rather than a recall claim. Also passed:

```bash
cargo test --lib structural_score_ranks_less_damaged_matrix_first --all-features
cargo test --test conformance_matrix_tests --all-features
QR_MAX_DIM=800 cargo test --test decode_regression_tests --release --all-features -- --ignored --nocapture
```

**2026-07-12 matrix-latency evidence:** Added the reproducible Criterion
benchmark `decode_matrix/canonical_v1_m_numeric` in `benches/matrix_decode.rs`.
It parses a checked-in clean V1-M numeric conformance matrix, checks its exact
raw payload once, and times only the public `QrDecoder::decode_matrix` call.
On Apple Silicon with Rust 1.85.0, matched isolated `git archive` snapshots
measured **6.9570–6.9782 us** (95% CI) before WP-007 at `82fc517` and
**6.3762–6.3959 us** at current measured snapshot `e41cb14`; the point
estimates show an **8.35%** reduction. Both used the identical fixture and
byte-for-byte copied Criterion closure; the older snapshot received only the
temporary benchmark wiring, not decoder changes. Full methodology, fixture
checksum, environment, and a noisier confirming alternating pair are in
`docs/wp007_matrix_latency.md`. This is a modest clean-matrix component
improvement, not proof that latency "dropped substantially": the roadmap has
no substantial-latency threshold. WP-007 therefore remains in progress pending
a defined threshold/evidence and the existing targeted photographic recovery
checks.

---

## WP-008: Introduce request-scoped configuration and diagnostics

**Status:** Completed (2026-07-13)

**Current evidence:** A public immutable `DecoderOptions` API now provides
fast/balanced/exhaustive deadline presets, a per-request diagnostics switch,
and `try_detect_with_options`. Optional diagnostics return `FailureStage` and
`DetectionTelemetry`; the no-diagnostics path does not collect telemetry.
Candidate and erasure budgets propagate through the diagnostics recovery path
and the legacy decoder/erasure counters have been removed. The library pipeline
now uses fixed library defaults plus request-scoped options rather than reading
`QR_*` process variables; the CLI retains environment translation only for its
benchmark/dataset controls. Reserved QR content modes are reported through the
diagnostic `FailureStage::UnsupportedContent` and telemetry counter rather than
being conflated with generic payload failure.

**2026-07-12 candidate-budget update:** `DecoderOptions` now carries an
immutable candidate decode-attempt limit: Fast/Balanced/Exhaustive allocate
16/128/512 attempts, and `with_candidate_limit` supplies an exact per-request
override. The diagnostics pipeline consumes this shared limit across its
brightness, binarization, contour, and ROI fallbacks; dense-scene routing can
redistribute but cannot expand the caller's cap. A zero limit also returns
before detection work on the no-diagnostics API path. Concurrent diagnostic
requests have isolated returned telemetry in a four-thread test. Passed:

```bash
cargo test --lib decoder_options_presets_are_immutable_and_ordered --all-features
cargo test --lib concurrent_diagnostic_requests_keep_telemetry_request_scoped --all-features
cargo test --lib zero_candidate_limit_skips_work_without_diagnostics --all-features
```

**2026-07-12 request-context update:** `DecoderOptions` now owns the bounded
confidence-guided RS-erasure budget (Fast/Balanced/Exhaustive: 4/16/64,
overrideable with `with_erasure_attempt_limit`). The one
`DecodeRequestContext` for an options-bearing request carries its deadline,
remaining erasure attempts, and all decoder/erasure counters through the
pipeline. The legacy `RS_ERASURE_GLOBAL_ATTEMPTS`, erasure `thread_local!`,
and decoder/deadline `thread_local!` state are removed, so concurrent requests
cannot reset, consume, or report one another's recovery state. Focused checks
passed: `cargo fmt -- --check`, `cargo test --lib --all-features` (113 passed),
`cargo test --test timeout_api_tests --all-features`, and
`cargo test --test conformance_mutation_tests --all-features`.

**2026-07-13 cancellation audit:** Added the immutable, cloneable
`CancellationToken` and `DecoderOptions::with_cancellation`. Its signal is
carried into the request's `DecodeRequestContext`, so matrix recovery checks
and image-pipeline deadline checkpoints stop only the request holding that
token. Diagnostic requests report `FailureStage::Cancelled`; an independently
configured concurrent request remains unaffected. Passed:

```bash
cargo test --lib cancellation_is_request_scoped_and_reported --all-features
```

The option API now satisfies the request-isolation, environment-independence,
and structured-failure acceptance criteria. Validation on 2026-07-13 passed
`cargo fmt -- --check`, strict all-target Clippy, and `cargo test --all-features`
(121 library tests and all integration tests).

**Goal:** Replace global environment-driven and thread-local behavior with a
production-quality library API.

**Tasks:**

- Add immutable `DecoderOptions` or equivalent.
- Support fast, balanced, and exhaustive presets.
- Make deadline, cancellation, candidate limits, erasure budgets, and telemetry
  request-scoped.
- Remove process-wide counters that allow concurrent decodes to interfere.
- Return structured diagnostics and failure stages when requested.
- Keep diagnostic collection optional and cheap when disabled.

**Acceptance criteria:**

- Concurrent requests cannot reset or consume one another's budgets.
- Library behavior does not depend on process environment variables unless a
  CLI explicitly translates them into options.
- Callers can distinguish invalid input, unsupported content, timeout,
  detection miss, geometry failure, RS failure, and payload failure.

---

## WP-009: Create a false-positive and adversarial suite

**Status:** Safety foundation in progress; synthetic false-positive baseline
reported, broader annotated-corpus gate remains open

**Goal:** Prevent recovery heuristics from trading recall for hallucinated
payloads.

**Tasks:**

- Assemble licensed negative images containing text, checkerboards, packaging,
  screens, Data Matrix, Aztec, linear barcodes, finder-like graphics, and
  random noise.
- Add malformed and adversarial QR-like matrices.
- Track false positives per image and per megapixel.
- Fuzz matrix parsing, block deinterleaving, RS correction, payload parsing, and
  public image APIs.
- Add sanitizer, Miri where applicable, and long-running fuzz workflows.

**2026-07-12 safety foundation:** Added deterministic adversarial matrix tests
for QR-sized seeded noise, malformed dimensions, and finder/timing damage past
the documented recovery tolerance. The existing structural-invalid fixtures
remain the source for invalid format/version BCH words, remainder bits, and
payload padding. Added the bounded `fuzz/matrix_decode` target alongside the
public image-input target; it covers matrix dimension checks, format/version
parsing, block deinterleaving, RS correction, payload parsing, and erasure
evidence. A weekly/manual GitHub Actions sanitizer workflow runs both targets
for a bounded duration and uploads minimized failures. `cargo fuzz` could not
be compiled locally because this environment cannot resolve `index.crates.io`
for `libfuzzer-sys`; the workflow is the first networked validation.

**2026-07-13 synthetic negative baseline:** Added the self-authored,
deterministic nine-image corpus in `tests/negative_corpus/manifest.json` and
its source renderer/test in `tests/negative_image_corpus_tests.rs`. It covers
synthetic block text, checkerboard, package-panel, screen-grid,
Data-Matrix-like, Aztec-like, linear-barcode-like, finder-like, and seeded-noise
patterns; it does not claim external photographs, screenshots, packaging, or
valid examples of those barcode formats. The public grayscale API evaluator
reported 0 positive images and 0 false-positive detections across 589,824
pixels (0.589824 MP): 0.0 false positives/image and 0.0 false
positives/megapixel. Reproduce it with
`python3 scripts/evaluate_negative_corpus.py --output /tmp/negative-corpus.json`.
This is a reproducible synthetic baseline, not a production FPR budget: the
broader licensed, annotated corpus and agreed budget remain required.

Focused local validation:

```bash
cargo fmt -- --check
cargo test --test adversarial_matrix_tests --all-features
cargo test --test input_api_tests --all-features
python3 scripts/evaluate_negative_corpus.py --output /tmp/negative-corpus.json
git diff --check
```

**Acceptance criteria:**

- False-positive rates are reported alongside recall.
- Recovery changes cannot merge when they exceed the agreed false-positive
  budget.
- Fuzz failures are reproducible and stored as regression fixtures.

---

## WP-010: Rebuild finder detection around ranked proposals

**Status:** In progress; initial ranked finder-proposal boundary completed

**Goal:** Replace repeated global scans and fallback combinations with a fast,
explainable proposal pipeline.

**Tasks:**

- Implement allocation-light horizontal and vertical scanline state machines.
- Estimate module pitch and useful scale ranges early.
- Rank finder candidates by ratio, cross-check, contrast, quiet zone, and local
  geometry.
- Group candidates without fixed pixel-distance constants.
- Use proposal-level non-maximum suppression and preserve multiple symbols.
- Add ROI-first processing for dense scenes.
- Measure stage-level recall and latency, not only final decoding.

**2026-07-12 initial proposal slice:** `FinderDetector::detect_proposals`
now exposes ranked, module-scale NMS'd single-finder proposals independently
of triplet grouping and geometry. Each proposal records horizontal/vertical
1:1:3:1:1 ratio agreement, pitch agreement, local binary contrast, and a
weak quiet-zone signal; the report records scanline and suppression counts.
The legacy `detect` API consumes this report, so decoder callers retain their
existing interface while proposal-stage tests can measure evidence directly.
This slice intentionally does not claim the full WP-010 acceptance criteria:
the scan state machines still allocate per row/column, grouping remains in the
legacy pipeline, and ROI-first/dense-scene stage evaluation are pending.

**2026-07-12 module-scaled grouping follow-up:** the legacy grouping guard no
longer rejects triples because their finder centres exceed a global 3,000-pixel
distance. Its upper span is now expressed in modules, with Model 2 version-40
diagonal and perspective headroom. Focused regressions prove a uniformly
upscaled valid symbol still groups and an impossible module span is rejected.
This removes one fixed-pixel constraint from the proposal-to-group boundary;
it does not yet replace the legacy grouping implementation or provide
ROI-first raster processing.

**2026-07-13 stage-evaluation evidence:** qrtool proposal-eval now measures
proposal and grouping stages independently against scaled BoofCV quadrilateral
labels, before transform or payload decoding can hide a failure. Its
containment contract and recorded artifacts are documented in
docs/wp010_stage_evaluation.md. At QR_MAX_DIM=800, nominal finder/group recall
is 71/78 (91.03%)/59/78 (75.64%), while dense lots is 80/420 (19.05%)/3/420
(0.71%). This makes the proposal boundary measurable, not complete: ROI-first
raster processing, allocation-light scanline state machines, region-aware
grouping, full-corpus evidence, and agreed category gates remain outstanding.

**2026-07-13 spatial grouping follow-up:** Dense module-size buckets now take
a bounded, region-aware path rather than a scene-wide cubic expansion. It
first preserves isolated three-finder local components using each proposal's
nearest compatible scale as the routing radius, then evaluates bounded
scale-compatible neighborhoods and retains the best remaining geometry. The
controlled 25-symbol grouping regression keeps every symbol's own finder
triple. On the existing `lots` stage evaluator at `QR_MAX_DIM=800`, grouping
recall improves from 3/420 (0.71%) to 52/420 (12.38%), with proposal recall
unchanged at 80/420 (19.05%) and grouping p95 1.322 ms. This is a measurable
grouping improvement, not WP-010/WP-012 completion: raster ROI rescanning,
finder recall, decoded dense-scene recall, and category gates remain open.

Verified with:

```bash
cargo test --lib detector:: --all-features
cargo clippy --all-targets --all-features -- -D warnings
```

**Acceptance criteria:**

- Finder and grouping recall can be evaluated independently.
- Common images avoid unnecessary full-image fallback scans.
- Multi-QR scenes do not discard candidates after the first valid triple.
- No targeted category regresses beyond its agreed gate.

---

## WP-011: Geometry and sampling refinement

**Status:** In progress; bounded sampling-confidence and alignment-ranking slice

**Goal:** Improve high-version, perspective, curved, rotated, and small-module
decoding without unbounded offset searches.

**Tasks:**

- Refine homographies using timing and alignment residuals.
- Use all relevant alignment patterns for high versions.
- Add bilinear or confidence-aware subpixel sampling.
- Scale sample footprints to module pitch.
- Use local sampled-grid thresholds under uneven lighting.
- Add a bounded nonlinear mesh model for curved surfaces.
- Model saturation/glare masks and avoid treating clipped pixels as confident.

**2026-07-12 bounded sampling slice:** The selected proposal's sampling path
now preserves a single bilinear center sample when local module pitch is below
1.5 pixels instead of forcing a 3x3 footprint that blends neighboring modules.
It also records the fraction of bright-clipped samples in each module footprint
and reduces that module's confidence to zero when the footprint is fully
saturated, while retaining its sampled bit for normal decoding. The existing
confidence-aware RS path can therefore treat glare as uncertainty rather than
as strong white evidence. Alignment transform ranking now averages every
spec-relevant alignment-pattern residual (at most 46 at version 40) rather
than using only the far-corner pattern; the nine transform candidates remain
fixed and bounded. This does not claim full WP-011 completion: timing/alignment
homography fitting, curved-scene evidence, category gates, and candidate-level
deadline telemetry remain pending.

Focused validation passed:

```bash
cargo test --lib decoder::qr_decoder::geometry::tests --all-features
cargo clippy --all-targets --all-features -- -D warnings
```

Category probes at `QR_MAX_DIM=800` deliberately remain evidence rather than
a performance claim: `high_version --limit 3` decoded 0/3 and `glare --limit
3` decoded 1/3. The probes confirm the bounded path executes on the intended
categories; their misses show that this slice alone does not satisfy the
remaining homography/refinement and glare-recovery work.

**2026-07-13 bounded homography slice:** Grayscale decoding now refines the
finder-derived transform against timing contrast and alignment residuals. It
probes at most six deterministic alignment locations and nine sub-module
offsets per observed location; each candidate is ranked by both timing lines
and every spec-relevant alignment pattern, then replaces the original only on
a measurable score improvement. The geometry unit probe builds a synthetic
v7 perspective grid with timing and alignment patterns and verifies that the
bounded fit improves the combined residual. This is not curved-surface
completion: the mesh path remains disabled after its zero-success benchmark,
and category-level gates for high_version, perspective, curved, rotations,
brightness, bright_spots, glare, and shadows still need reproducible
before/after evidence.

**Acceptance criteria:**

- Improvements are demonstrated separately for `high_version`, `perspective`,
  `curved`, `rotations`, `brightness`, `bright_spots`, `glare`, and `shadows`.
- Candidate refinements are ranked and bounded by deadline.
- Clean nominal images do not pay the full recovery cost.

---

## WP-012: Dense multi-QR detection

**Status:** In progress; controlled raster baseline exposes dense-scene gap

**Goal:** Make the seven `lots` images and realistic dense scenes first-class,
not edge cases.

**Tasks:**

- Implement one-to-one proposal grouping and result deduplication.
- Suppress decoded regions while preserving overlapping candidates.
- Add spatial indexing for finder and symbol proposals.
- Report symbols/second and recall as scene density rises.
- Create controlled scenes with 1, 2, 5, 10, 25, 50, and 100 symbols.

**2026-07-12 initial dense-scene slice:** Accepted decoded symbols now own
their three finder proposals, rather than allowing a later candidate triple to
reuse any of them. Ownership is assigned only after a successful decode, so a
false high-ranked triple cannot starve a valid neighbour. Result deduplication
is geometry-based: duplicate observations of the same region are suppressed,
while physically distinct symbols carrying identical payloads are retained.
Proposal-stage controlled scenes cover 1, 2, 5, 10, 25, 50, and 100 symbols
and assert that module-scaled NMS preserves each symbol's three finders.
Focused checks passed:

```bash
cargo test --lib pipeline::tests --all-features
cargo test --lib detector::proposal::tests --all-features
```

**Remaining scope:** This is not yet dense-scene completion. It still needs a
spatial index for grouping/region routing, rasterized multi-QR end-to-end
scenes with bipartite geometry evaluation, measured density recall/throughput,
and explicit duplicate/false-positive budgets. The 50-code 500 ms target is
not claimed by this slice.

The module-scaled grouping follow-up above also applies to dense raster scenes:
large images no longer discard a valid local symbol solely for being rendered
past a global coordinate limit. It is unit-level proposal/group evidence only,
not the required controlled raster-scene throughput result.

**2026-07-13 controlled raster baseline:** Added deterministic, labeled
version-1 EC-M scenes at 1, 2, 5, 10, 25, 50, and 100 symbols (193 total),
with a manifest that records the `python-qrcode==11.1.0` backend and pixel/
label hashes. `scripts/evaluate_wp012_raster_scenes.py` evaluates every scene
through the public `qrtool reading-rate` path. Its localization metric is
IoU >= 0.5 maximum-cardinality bipartite geometry matching, so this is
end-to-end result recall rather than a count-based claim. The native-resolution
release baseline is 14/193 (7.25%) with zero false positives/duplicates and
782.61 ms mean image runtime. The 50-symbol scene is 2/50 (4.00%) at
3607.59 ms (13.86 symbols/s), far short of the 90%/<500 ms target. See
`docs/wp012_raster_scenes.md` and
`artifacts/wp012_controlled_dense_by_density_qrmax0.json`.

**Acceptance criteria:**

- Evaluator performs bipartite geometry matching.
- At least 90% recall on a controlled 50-code scene under 500 ms is the
  long-term target.
- Duplicate results and false positives remain within explicit budgets.

---

## WP-013: Establish the OSS competitor harness

**Status:** Harness complete; pinned adapter builds remain environment setup

**Goal:** Compare RustQR fairly against current open-source alternatives.

**Initial adapters:**

- ZXing-C++
- quirc
- ZBar
- BoofCV
- OpenCV `QRCodeDetector`
- a maintained Rust-native decoder

**Tasks:**

- Pin exact versions and build settings.
- Feed every implementation the same decoded pixel buffers.
- Enforce matched thread counts, timeouts, resolution, and preprocessing.
- Normalize raw payload and metadata without hiding charset differences.
- Publish adapter source and raw artifacts.
- Record license and redistribution constraints.

**Acceptance criteria:**

- Competitor claims are locally reproducible.
- Accuracy and latency use identical case manifests and timing boundaries.
- Results include confidence intervals and hardware metadata.

**Completion record (2026-07-12):** Added `competitors/lock.json`, locally
authored quirc/rqrr/BoofCV adapters, `scripts/run_competitor_harness.py`, and
`docs/competitor_harness.md`. The lock pins ZXing-C++ v2.3.0, quirc v1.2,
ZBar 0.23.93, BoofCV v1.1.7, OpenCV 4.12.0, and rqrr 0.9.0, with source,
revision, release build settings, licenses, and redistribution limits. The
harness converts every selected input once to a temporary normalized PGM and
gives byte-identical pixels, one thread, one timeout, and identical timed
adapter invocation boundaries to every selected adapter. Reports contain the
case manifest, image/pixel checksums, raw stdout/stderr bytes, normalized
payload hex, explicit unavailable/version-mismatch/timeout states, bootstrap
median confidence intervals, and hardware metadata. The intentionally thin
CLI protocol declares adapter metadata unavailable rather than fabricating
charset, ECI, EC-level, or geometry fields; annotation-count coverage is
likewise labelled as non-localization/non-payload accuracy.

Local setup evidence: OpenCV 4.12.0 was detected and decoded the one-image
monitor smoke lane; locally installed ZBar 0.23.93 was detected but returned
no decode for that image. ZXing-C++, quirc, BoofCV, and rqrr wrappers are
recorded as unavailable until their pinned builds are placed in
`competitors/bin/`. Passed:

```bash
python3 -m unittest scripts/tests/test_run_competitor_harness.py
python3 scripts/run_competitor_harness.py --category monitor --limit 1 \
  --adapter zbar --adapter opencv --timeout-ms 5000 \
  --output /tmp/competitor-smoke.json
git diff --check
```

---

## WP-014: Performance engineering after correctness

**Status:** Groundwork recorded; performance changes remain blocked by WP-007,
WP-010, a successful multi-code lane, and trustworthy matched benchmarks

**Goal:** Reach OSS-leading latency without sacrificing verified correctness.

**Tasks:**

- Profile successful clean, hard, and multi-code cases separately.
- Remove per-row and per-candidate allocations.
- Reuse request-scoped buffers safely.
- Measure when Rayon helps and avoid parallel overhead for small images.
- Add architecture-specific SIMD behind tested feature gates.
- Measure cold/warm latency, allocations, RSS, and sustained throughput.
- Evaluate PGO only after representative workloads are stable.

**Targets:**

- Desktop p50 no more than 25 ms and p95 no more than 100 ms at max dimension
  1024 for the near-term credibility gate.
- Sustained 30 FPS on representative 1080p single-code video.
- Beat the fastest tested OSS implementation, or remain within 10% while
  achieving at least three percentage points higher recall.

**2026-07-12 groundwork evidence:** Added a reproducible local baseline
collector, `scripts/collect_wp014_baseline.py`, and its allocation-aware
Criterion probe, `benches/wp014_profiles.rs`. The probe separates loaded-RGB
`detect` timing/allocation/throughput from fresh-process end-to-end timing and
sampled RSS. Its fixed clean (`nominal/image005.jpg`) and hard
(`damaged/image002.jpg`) lanes currently decode; the dense `lots/image001.jpg`
control currently returns zero symbols and is deliberately marked as a
negative control rather than a successful multi-code performance result. The
allocation-only smoke (`WP014_ALLOCATION_ONLY=1 WP014_PROFILE_ITERATIONS=1
cargo bench --bench wp014_profiles --features tools -- --noplot`) recorded
first-call allocation counts of 13,711 (clean), 758,042 (hard), and 17,561,330
(multi candidate). See `docs/wp014_baseline.md` for boundaries, commands, and
remaining gates. No target claim or detector optimization is implied.

---

## WP-015: Platform and packaging roadmap

**Status:** In progress — feature separation and package metadata landed; core
extraction and non-desktop support remain gated

**Goal:** Turn the decoder into an adoptable OSS product.

**Tasks:**

- Separate a small `no_std + alloc` matrix-decoding core if feasible.
- Make image loading, Rayon, tools, and platform SIMD optional features.
- Add MSRV, WASM, Linux, macOS, Windows, iOS, and Android CI lanes as support is
  implemented.
- Provide C ABI, Swift, Kotlin, Python, and WASM bindings in staged releases.
- Publish crates.io releases, tags, API documentation, examples, changelog,
  semver policy, security policy, and dataset provenance.

**Acceptance criteria:**

- Platform claims correspond to continuously tested build or runtime lanes.
- Optional dependencies do not leak into the core configuration.
- Bindings share conformance fixtures with the Rust API.

**2026-07-12 packaging/core assessment:** The current crate cannot truthfully
offer `no_std + alloc`: public request options, diagnostics, image pipeline,
and decoder timing use `std`, so extracting only selected modules would not
create a usable supported core.  Instead, the package now declares MSRV 1.85
and separates Rayon (`parallel`), private grayscale SIMD (`simd`), and image
loading (`image-loading`) into optional features. Default behavior remains
`parallel + simd`; scalar fallbacks preserve the public helper APIs with
`--no-default-features`. `tools` owns the optional image loader and CLI.

CI now checks the minimal feature set and MSRV in addition to its hosted
Linux/macOS/Windows test lanes. This is build/test evidence only: WASM, iOS,
Android, bindings, and `no_std` remain explicitly unsupported/planned until
dedicated extraction and continuous target lanes exist.

**Validation:**

```bash
cargo test --lib --no-default-features
cargo test --lib --all-features
cargo clippy --all-targets --all-features -- -D warnings
```

---

## WP-016: Closed-SDK challenger features

**Status:** Long-term; begin only after OSS-leading correctness

**Goal:** Differentiate RustQR through transparency, control, and diagnostics,
not only aggregate recall.

**Candidate features:**

- Calibrated confidence.
- Per-stage diagnostic traces.
- Damaged-module, glare, and sampling-confidence heatmaps.
- Corners, pose, module pitch, and geometric uncertainty.
- QR print-quality grading.
- Structured Append assembly across images or video frames.
- Video ROI tracking, reacquisition, and duplicate suppression.
- Deterministic private/offline operation.
- Fast, balanced, and forensic modes with hard service-level objectives.

**Long-term target:**

- Stay within two recall percentage points of the best tested commercial SDK.
- Beat it in at least half of named hard categories.
- Beat it on p50 latency or memory in a matched one-thread configuration.
- Publish evaluator source, manifests, raw artifacts, and confidence intervals.

---

## Current project snapshot

- Branch: `main` at `5b9b41e`; the dirty working tree is the active roadmap
  implementation and must be preserved rather than treated as disposable.
- The untracked `benchmark/` directory contains preserved historical evidence;
  preserve it.
- `cargo test --all-features`: passed 94 library tests, 5 CLI tests, 6
  conformance matrix tests, 3 mutation tests, 7 input API tests, 2 timeout
  tests, and one doc test; seven real-image integration tests were ignored.
- `cargo fmt -- --check`: passed.
- `cargo clippy --all-targets --all-features -- -D warnings`: passed.
- Main README benchmark: 18.26% overall instance reading rate, median about
  820 ms/image, GitHub run `21837108650`.
- Untracked run `21927167412`: custom payload lane reported 26/26 at median
  156.7 ms; BoofCV semantics and provenance require WP-001 verification.
- Rebuild branch `work_wp014_wp018_gates_2026_02_12` was 33 commits ahead of
  `main` and must be evaluated through WP-005 rather than merged blindly.

## Suggested prompt for a new session

> Read `AGENTS.md` and `TODO.md`, then work only on WP-XXX. Launch focused
> sub-agents to inspect implementation, tests, and benchmark implications.
> Preserve the untracked benchmark artifacts. Use targeted local benchmark
> limits during development and GitHub Actions for full runs. Implement the
> packet, validate its acceptance criteria, and update `TODO.md` with results
> and remaining work.

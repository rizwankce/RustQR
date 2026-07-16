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

**Status:** Completed (2026-07-13); retain current Model 2 implementation

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

**Normalization audit (2026-07-13):**

- Re-inspected `main`, `scratch_from_scratch_rebuild` (`295b97c`), the current
  `qrtool`, and the Fast Benchmark workflow. The rebuild retains image-space
  corners internally, but its CLI artifact records only image-level match
  booleans and runtime. Its `wp007-reading-rate-v1` evaluator treats a BoofCV
  annotation image as a success if any code is returned; it cannot be
  retrospectively converted to one-to-one localization, false-positive, or
  duplicate metrics. `main` likewise does not emit v2 artifacts.
- No fresh benchmark claim was made. The ADR now names the required throwaway
  adapters, exact prediction-stream fields, seven-category local sequence,
  comparator artifacts, and the matched macOS Fast Benchmark dispatch. This
  is a source/contract audit, not the required normalized comparison itself.
- WP-005 remains **Direction decided; normalized cross-branch v2 benchmark
  pending**. The next implementation must produce adapter artifacts before
  running either local or Actions comparisons; historical v1 results remain
  diagnostic only.

**2026-07-13 adapter-contract groundwork:** Added the external throwaway
`rustqr.wp005.prediction-stream.v1` contract and
`scripts/normalize_wp005_prediction_stream.py`. A branch adapter must export
per-image identity, original/working dimensions, every original-coordinate
prediction quadrilateral, optional payloads, core/end-to-end times, timeout,
and matching dataset/preprocessing fingerprints; one shared truth manifest
provides the labels and label fingerprint. The normalizer validates coverage,
dimensions, and fingerprints, then applies the current v2
`quad-iou=0.5;matching=max-cardinality` score and emits compatible v2 summary
and category fields including false positives, false negatives, duplicates,
and timeouts. Synthetic parser tests cover duplicate/timeout scoring and
dimension rejection. No `main`, rebuild, or current all-branch stream exists
yet, so this is contract/tooling preparation only—not a normalized comparison,
an accuracy result, or a latency claim. See `docs/wp005_prediction_stream.md`.

**2026-07-13 current exporter smoke:** `qrtool prediction-export` now writes
the documented `rustqr.wp005.prediction-stream.v1` directly from the current
branch using the existing image loader, detector, and cooperative timeout
path; it only serializes original-coordinate predictions and does not alter
decoder behavior. `scripts/make_wp005_truth_manifest.py` builds the separate
shared BoofCV `SETS` truth manifest for exactly the exported images. The
one-image 1024-px nominal artifacts
`wp005_current_nominal_limit1_prediction_stream.json`,
`wp005_truth_nominal_limit1_1024.json`, and
`wp005_current_nominal_limit1_v2.json` normalized successfully. That image
exported zero predictions and consequently scores 0/2, so this proves only
the current adapter/scorer handoff—not recall, latency, or any cross-branch
comparison. The main/rebuild throwaway adapters and matched 25-image exports
remain required.

**2026-07-13 adapter portability audit:** The current `prediction-export`
subcommand cannot be mechanically cherry-picked as-is: `main` needs
original-dimension capture around its existing geometry-less loader, and the
rebuild uses a separate hand-written CLI plus `payload`/`corners` fields
instead of `content`/`position`. Both expose sufficient public `detect` data
for temporary standalone exporter bins. The executable, branch-neutral setup
is documented in `docs/wp005_prediction_stream.md`: create detached disposable
worktrees at `main` `5b9b41e` and rebuild `295b97c`, add only a matching
throwaway adapter bin, export the same 25-image 1024px slice with timeout zero,
then verify image identity/fingerprints before one shared-truth normalization.
No worktree was created or altered and no comparison was claimed. This removes
the interface-discovery blocker; reviewed adapters and actual exports remain
pending.

**2026-07-13 pinned-adapter smoke:** Detached disposable worktrees at
`main@5b9b41e` and `scratch_from_scratch_rebuild@295b97c` now each built a
reviewed standalone `wp005_prediction_export` bin from
`scripts/wp005_adapters/`. Both exported the exact same one-image
`nominal/image001.jpg` slice, with matching image identity, FNV dataset
fingerprint, and 1024px Triangle preprocessing fingerprint; one shared truth
manifest normalized both valid streams with the v2 scorer. This proves the
historical API adapters and shared-normalization handoff. It is not a
cross-branch result: main's public fast path returned a decoded payload with
an all-zero position, which the adapter deliberately dropped rather than
fabricating a quadrilateral, and both one-image localization scores were 0/2.
The required seven-category 25-image-per-category export, geometry-capable
main result, matched Fast Benchmark run, and comparison table remain pending.

**2026-07-13 main geometry projection probe:** `main@5b9b41e` retains the
measured finder-triplet geometry that generated a successful decode, although
its public `QRCode.position` is left at zero. A reviewed patch retained under
`scripts/wp005_adapters/` projects only that candidate's outer boundary; it
does not invent payload-derived boxes and is for the detached historical
worktree only. It normalized at 17/17 hits with zero false positives, false
negatives, duplicates, or timeouts on the monitor directory. The probe also
found that passing a category directory as exporter root makes each filename a
category, defeating the intended one-image limit. The required comparison
remains pending: run from the BoofCV root with the seven named categories,
25 images per category, matching image IDs/fingerprints, shared truth, and the
same Fast Benchmark configuration. The reviewed historical adapters still
lack a category allow-list, so that exact seven-category slice is blocked
until all throwaway adapters share one (or use the same materialized input
tree). Evidence and exact patch instructions:
`artifacts/wp005_main_geometry_monitor_2026-07-13.md`.

**2026-07-13 bounded nominal normalized comparison:** Both retained historical
adapters now require the same explicit repeated category allow-list and accept
an optional deterministic per-category offset for bounded sharding. Five
non-overlapping five-image shards per ref gave the same complete 25-image
`nominal` selection at 1024 px, matching IDs and fingerprints, then one shared
truth manifest and v2 scorer. `main@5b9b41e` and rebuild `295b97c` each hit
18/29 labels (62.07%) with 0 false positives, duplicates, and timeouts;
observed local median end-to-end times were 1690.258 ms and 130.058 ms. The
normal comparator correctly fails this partial artifact because the required
`rotations`, `high_version`, and `lots` gates are absent. This does not change
the direction decision or satisfy the seven-category/Actions acceptance gate.
Exact raw, normalized, and caveat evidence:
`artifacts/wp005_nominal_cross_branch_2026-07-13.md`.

**2026-07-13 complete local normalized comparison:** The same detached,
historical adapters and shared v2 scorer now cover all seven requested
categories from matching 1024px pixels: 25 deterministic images per category,
except all 7 available `lots` images (157 images / 727 labels total). The
streams have identical ordered image IDs, dimensions, dataset fingerprint
`fnv1a64:812bbf57f2ce9c36`, and Triangle preprocessing fingerprint. `main`
hit 36/727 labels (4.95%; one false positive) and rebuild hit 74/727 (10.18%;
zero false positives); neither emitted duplicates or timeouts. Rebuild gained
in rotations (+27 labels) and perspective (+10) but lost main's one `lots`
label. The two historical release binaries had locally observed median
end-to-end times of 2167.191 ms and 192.968 ms respectively; that is
diagnostic timing, not a cross-run performance claim. This closes the local
normalized-stream gap but does **not** satisfy the remaining matched remote
Fast Benchmark gate or authorize a rebuild merge/transplant. Exact raw,
truth, normalized, checksum, category-table, and reproduction evidence:
`artifacts/wp005_targeted_cross_branch_2026-07-13.md`.

**2026-07-13 matched macOS Fast Benchmark dispatch:** The required workflow
ran successfully for both pinned historical implementations at 1024 px across
the same seven categories and 25-image per-category limit (all seven `lots`
images): main run `29241027260` and rebuild run `29240862028`. The rebuild
required a temporary workflow-only compatibility wrapper because its historical
CLI predates `--non-interactive`/`--artifact-json`; its final wrapper commit
`3f2ab45` adds no decoder code and invokes the seven historical profiles
separately. Main reports `rustqr.reading_rate.v1` label counts while rebuild
reports `wp007-reading-rate-v1` image booleans, so the remote outputs verify
the matched macOS configuration and timing boundaries but are not an accuracy
delta. The detached shared-v2 comparison above remains the decision evidence:
keep the current Model 2 branch, do not merge/transplant rebuild code, and
only revisit quarantined ideas one reversible slice at a time. Checksums,
per-category results, workflow provenance, and artifact links are retained in
`artifacts/wp005_fast_benchmark_runs_2026-07-13.md`.

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

**2026-07-13 acceptance audit:** A repository-wide audit found no numeric or
otherwise objectively evaluable definition of “drops substantially.” At
`47319a4`, two immediate local `cargo bench --bench matrix_decode` runs gave
95% confidence intervals of 8.7282–9.1043 us and 9.2808–10.071 us for the
same canonical fixture. The variance on the shared machine makes those runs a
current local range, not a valid comparison with the isolated archive pair
above and not a performance-direction claim. The documented
`QR_MAX_DIM=800 cargo test --test decode_regression_tests --release
--all-features -- --ignored --nocapture` scope completed its seven test
functions, but only monitor, rotated, and a five-result multi-code smoke
produced output; blurred, nominal, damaged, and high-version inputs warned of
no decode. The tests deliberately allow those warnings and do not assert
expected payloads or expected multi-code counts, so they are safety/smoke
evidence rather than proof of recall preservation. WP-007 cannot be marked
complete until a measured latency threshold and payload/count-asserting
photographic recall gate exist and pass.

**2026-07-13 photographic regression strengthening:** Replaced the seven
warning-only ignored image tests with a strict, label-backed
`monitor_image001_decodes_payload_and_localizes_label` gate. At a fixed 800px
maximum dimension it requires exactly one result, raw/text payload
`4376471154038`, Model 2 version 1, EC-L, and one-to-one localization at IoU
>= 0.5 against the checked-in BoofCV annotation, with no false positives or
duplicates. BoofCV provides geometry/count labels but no payload labels, so
the remaining blurred, high-version, rotations, damaged, lots, and nominal
fixtures are retained as explicitly unresolved label-validated scope (expected
counts 1, 1, 3, 1, 60, and 2 respectively), not warning-only passing decoder
tests. This creates one legitimate strict photographic regression but does not
prove the packet-wide no-recall-loss acceptance criterion.

**2026-07-13 second strict photographic gate:** Added the successful labeled
`close/image002` fixture to the ignored release regression command. At the
same fixed 800px Triangle resize it requires exactly one Model 2 version-7
EC-L result, its stable raw/text payload, and one-to-one IoU >= 0.5
localization against the BoofCV annotation, with no false positives or
duplicates. The ignored release suite passes both strict gates. The known
blurred, high-version, rotations, damaged, lots, and nominal shortfalls remain
explicitly unresolved; this is stronger successful-case evidence, not a
packet-wide recall-completion claim.

**2026-07-13 strict-path photographic evidence:** The two successful strict
fixtures now also run through `try_detect_with_options` with request-scoped
diagnostics. At the same 800px policy, both monitor/image001 (V1) and
close/image002 (V7) decode exactly once with zero matrix recovery-mode attempts
and zero RS-erasure attempts. This proves those two passing photographic paths
do not depend on the bounded decoder recovery frontier; it neither establishes
category-wide recall nor changes the unresolved fixtures above. Passed:

```bash
cargo test --release --test decode_regression_tests --all-features \
  strict_path_needs_no_decoder_recovery -- --ignored --nocapture
```

**2026-07-13 clean-matrix path guard:** The checked-in canonical V1-M matrix
now has a request-local decoder-path regression test. It requires one strict
canonical payload attempt, one RS candidate, and zero bounded-recovery payload
attempts. This directly proves the clean public matrix API returns before the
bounded recovery frontier; the existing Criterion benchmark remains the timing
evidence. Passed:

```bash
cargo test --lib clean_matrix_uses_one_strict_payload_path_without_recovery --all-features
```

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

**Status:** Safety foundation in progress; synthetic and licensed ZXing
high-contrast false-positive baselines reported, broader representative-corpus
gate remains open

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

**2026-07-13 fuzz replayability audit:** The fuzz inputs remain bounded by the
targets (public input dimensions at most 32; matrices at most 177x177 plus a
same-size confidence sidecar). Local `cargo fuzz` remains unavailable here
because `libfuzzer-sys` requires registry access. The scheduled/manual
sanitizer workflow now uses `pipefail`, retains a per-target sanitizer log on
success or failure, and continues uploading any minimized crash inputs. This
makes remote campaign output and crash replay inspectable without claiming a
local sanitizer run or production FPR evidence.

**2026-07-13 synthetic negative baseline:** Added the self-authored,
deterministic thirteen-image corpus in `tests/negative_corpus/manifest.json` and
its source renderer/test in `tests/negative_image_corpus_tests.rs`. It covers
synthetic block text, checkerboard, package-panel, screen-grid,
Data-Matrix-like, Aztec-like, linear-barcode-like, finder-like, seeded-noise,
halftone, moire-like, nested-square, and QR-corner finder-triplet-with-broken-
timing patterns; it does not claim external photographs, screenshots, packaging,
or valid examples of those barcode formats. The expanded corpus has 655,360
pixels (0.655360 MP); the four added patterns each have focused zero-detection
public-API regressions. The public grayscale API evaluator reports aggregate
false-positive rates; reproduce it with
`python3 scripts/evaluate_negative_corpus.py --output /tmp/negative-corpus.json`.
This is a reproducible synthetic baseline, not a production FPR budget: the
broader licensed, annotated corpus and agreed budget remain required.

**2026-07-13 bounded evaluator update:** Every synthetic negative image now
uses `try_detect_with_options` with a request-scoped five-second deadline and
diagnostics. A `FailureStage::Timeout` increments `timeout_images` in the
stable metric line and fails the corpus test instead of being counted as a
zero-detection pass. The evaluator JSON preserves that field separately from
false-positive metrics. The focused bounded run completed 13/13 cases with
zero timeouts and zero detections; its transient report is
`/tmp/wp009_negative_bounded.json`. This remains synthetic-only evidence and
the cooperative deadline is not a hard process interruption.

**2026-07-13 external-corpus intake audit:** No locally checked-in external
negative corpus can be admitted yet. `benches/images/boofcv` has positive QR
annotations and its local documentation lists sources but no image-level
license or redistribution grant; it must not be repurposed as negative evidence.
`docs/wp009_adversarial.md` now records the required per-asset admission fields
(source, immutable revision/date, license, redistribution constraint, SHA-256,
category, dimensions, and zero-QR annotation) plus the required FPR-budget
decision. This is a durable selection path, not a claim that an external corpus
or production FPR gate exists.

**2026-07-13 external-photo source rejection:** Open Images is a plausible
route for photographed text, screens, and packaging, but its published
metadata says image licensing must be verified per image and its image-level
labels cannot establish that a selected image contains zero QR symbols. It is
therefore not admitted on the dataset-level license alone. A future intake must
pin each image's source/license metadata and add a manual zero-QR annotation;
until then, use only the locally verified ZXing slices below for external
negative evidence.

**2026-07-16 Wikimedia Commons intake route:** Unlike Open Images, individual
Commons file-description pages can provide an immutable `oldid`, author,
dimensions, original-file checksum, and a file-specific redistribution license.
For example, the permanent pages for `Book shelf-use.png` (CC-BY-3.0,
oldid `828586508`) and `Computer Screen Monitor.jpg` (CC-BY-SA-4.0,
oldid `1131317639`) are evidence-backed candidates for photographed text and
screen categories respectively. This establishes a usable *per-asset* licensing
route, not automatic admission: every candidate must pin its oldid and original
URL, retain the applicable license/attribution requirements, visually inspect
the original for zero QR symbols, then record a local SHA-256 and dimensions in
the external manifest before it can run as a gate.

**2026-07-16 Wikimedia screen admission:** The unmodified `Computer Screen
Monitor.jpg` original is now vendored as a one-image photographed-screen slice.
`wikimedia_commons_manifest.json` pins the original URL, permanent oldid,
author (`U3211603`), Commons source SHA-1, local SHA-256, dimensions
(4624x3468), CC-BY-SA-4.0 attribution, and a dated manual visual zero-QR
annotation. `scripts/verify_wp009_wikimedia_corpus.py` verifies each recorded
field that is locally testable, including the JPEG dimensions and local hash.
The strict public-RGB release gate completed with one image (16.036032 MP),
zero detections, and zero timeouts in 2.88 s; it terminated at a normal
Reed-Solomon rejection. An unoptimized debug run exceeded the cooperative
five-second request deadline, so this corpus gate is deliberately ignored and
release-only, like the strict cross-symbology gate. Reproduce it with
`python3 scripts/evaluate_negative_corpus.py --include-wikimedia`. This is a
representative photographed-screen *slice*, not a production FPR claim: photos
of packaging plus an agreed budget still remain open.

**2026-07-16 Wikimedia book-shelf admission:** The unmodified `Book
shelf-use.png` original is now a separate CC-BY-3.0 photographed-text slice;
it does not share the screen fixture's CC-BY-SA-4.0 manifest. Its manifest
independently pins the permanent oldid, author (`Valdes-and-Rauber`), Commons
source SHA-1, local SHA-256, dimensions (300x450), selected CC-BY-3.0 license,
and manual visual zero-QR annotation. The permanent source page additionally
offers GFDL, but this repository redistributes under its CC-BY-3.0 alternative.
The strict public-RGB release gate completed with zero detections/timeouts in
0.35 s. It too times out in an unoptimized debug build, so it is included only
in the ignored release qualification. `--include-wikimedia` now validates both
Commons manifests and runs both release gates. These two assets cover screen
and text categories; photographed packaging and an agreed production FPR budget
remain open.

**2026-07-16 Wikimedia packaging admission:** The unmodified `Packaging fragile
items for delivery.jpg` original is now a separate CC-BY-2.0 photographed-
packaging slice. Its manifest pins the permanent oldid, author (Meanwell
Packaging), Commons source SHA-1, local SHA-256, dimensions (4000x2667), and
manual visual zero-QR annotation; the source page records FlickrreviewR license
confirmation. The strict public-RGB release gate completed with zero
detections/timeouts in 1.23 s and a normal geometry rejection. Its debug run
exceeded the cooperative deadline, so it is included only in the ignored
release qualification. `--include-wikimedia` now verifies and qualifies all
three Commons fixtures. This completes the initial screen/text/packaging
category coverage, but it remains a three-image slice—not a representative
production corpus or an agreed FPR budget.

**2026-07-13 licensed ZXing corpus admission:** Vendored exactly 47 PNGs from
the Apache-2.0 `zxing/zxing` commit
`82333b3ed894ef097d41dd8c922689ede8880e01`: 22 `falsepositives` and 25
`falsepositives-2` fixtures. `tests/negative_corpus/zxing_manifest.json`
records each source/local path, SHA-256, dimensions, category, and explicit
`expected_qr_count: 0`; the bundled upstream Apache-2.0 license, NOTICE, and
REUSE declaration preserve provenance. `scripts/verify_wp009_zxing_corpus.py`
checks all 47 digests and PNG dimensions before the public RGB API evaluator
runs. Release verification in six deterministic shards covered 47/47 images
(12.518400 MP), with zero timeouts, positive images, or returned QR objects.
Timeouts fail rather than count as clean negatives. This is a licensed,
high-contrast-slice measurement, not a production FPR claim or completion:
photographed text, packaging, screens, representative non-QR symbologies, and
an agreed production false-positive budget remain open.

**2026-07-13 ZXing cross-symbology admission attempt:** Vendored exactly 17
Aztec, 23 Data Matrix, and 7 Code 128 PNG fixtures from the same immutable
Apache-2.0 ZXing revision. `zxing_cross_symbology_manifest.json` records each
asset's SHA-256, dimensions, upstream path, expected non-QR format, and zero
QR label; the integrity guard checks the fixed 17/23/7 suite membership as
well as hashes and dimensions. The bounded public-API evaluator keeps the
five-second per-image deadline and reports a separate metric. The initial
release run exposed timeouts on `zxing-aztec-hello`,
`zxing-aztec-hello-with-errors`, `zxing-aztec-lorem-105x105`, and
`zxing-aztec-lorem-151x151`; timeouts always fail rather than contributing
zero-detection evidence. The following bounded policy clears that gate without
weakening its deadline.

**2026-07-13 bounded small-input safety policy:** The cross-symbology timeout
traces reached finder/group/transform but never BCH, so decoder recovery was
not relevant. A global 32-candidate ceiling cleared the negatives but
regressed native controlled `dense_50` from 48/50 to 15/50 and was rejected.
The retained default instead caps the request-wide candidate schedule at 32
only when the *current processed input* has maximum side <=640; caller-supplied
stricter limits still win and larger multi-symbol inputs retain their existing
budget. The strict release cross test now passes all 47 images (5.068674 MP)
with zero detections and zero timeouts in 17.89 s. The existing synthetic and
ZXing negative suites remain zero-detection/zero-timeout, native dense_50 is
48/50 in 111.52 ms with no false positives, duplicates, or timeouts, and
matched label-backed nominal/rotation slices retained 5/9 and 7/15 recall.
Ignored strict monitor/close regressions and matrix conformance also passed.
This admits the cross-symbology slice as a bounded negative measurement; it
does not complete the representative-corpus or production-FPR-budget gates.

Focused local validation:

```bash
cargo fmt -- --check
cargo test --test adversarial_matrix_tests --all-features
cargo test --test input_api_tests --all-features
python3 scripts/evaluate_negative_corpus.py --output /tmp/negative-corpus.json
python3 scripts/verify_wp009_zxing_corpus.py
python3 scripts/verify_wp009_wikimedia_corpus.py
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

**2026-07-13 containment-evaluation follow-up:** `proposal-eval` artifact
schema v2 now records contained proposal/group counts and contained duplicate
groups, rather than reporting spurious counts without a denominator. On the
same `lots` labels at `QR_MAX_DIM=800`, the current bounded dense-routing
baseline retains 376 contained and 41 spurious proposals (417 after NMS), and
produces 83 contained groups: 76 first groups for distinct labels (18.10%
grouping recall), 7 contained duplicates, and 440 spurious groups. The
controlled 25-symbol grouping retention regression remains green. This makes
the remaining selectivity problem measurable; it does not change WP-010's
in-progress status or claim dense-scene acceptance.

**2026-07-13 proposal-boundary diagnosis:** `proposal-eval` schema v3 records
the number of annotations retaining zero, one, two, or at least three finder
centres and the number of three-centre annotations that never form a contained
group. This distinguishes proposal misses from grouping misses using the same
labels and bounded grouping path. The fresh `lots` artifact reports
263/36/41/80 symbols with 0/1/2/3+ centres respectively; just 4 of the 80
three-centre symbols fail to group. Proposal loss therefore explains 340 of
344 stage misses and establishes raster ROI proposal recovery—not more global
group tuning—as the next measured slice. This does not alter candidate routing
or claim an acceptance improvement.

**2026-07-13 bounded ROI proposal-recovery experiment:** Dense images with at
least 32 raw finder observations now choose at most twelve candidate-populated
192px cells and rescan only those cells with an exact edge gate. On `lots` at
`QR_MAX_DIM=800`, this improves finder/group recall from 80/420 and 76/420 to
81/420 (19.29%) and 77/420 (18.33%), raises contained proposals 376→377, and
does not add spurious proposals (41). The v4 artifact records 48 windows,
9,874 row scans, 10,284 column scans, and 790 supplemental observations over
seven images; a unit test proves the 12-window/216-row-or-column-per-window
cap. The controlled 1–100 scene evaluator retains 193/193 finder-stage
symbols. This is a bounded measured gain, not a sufficient dense recall or
latency improvement to complete WP-010.

**2026-07-13 bounded contour-family proposal supplement:** A dense scan now
runs one independent connected-component contour pass only after the primary
and bounded ROI scan. A contour candidate must independently pass horizontal,
vertical, and pitch evidence; it is appended after (not re-ranked ahead of)
the primary proposal order, remains NMS-distinct, and the appended frontier is
capped at 128. The QR_MAX_DIM=800 `lots` v6 artifact improves finder recall
81→102/420 (24.29%) and grouping 77→87/420 (20.71%), with 189 raw/112 appended
contour proposals, contained/spurious proposals 486/44, and 13.292ms proposal
mean latency. Controlled 1–100 scenes retain 193/193 finder-stage symbols.
This materially narrows the proposal gap but leaves 215 `lots` labels with no
contained proposal and does not complete WP-010.

**2026-07-13 proposal-evidence buckets:** The stage evaluator now retains
post-NMS finder evidence and reports fixed weighted-evidence bands separately
for contained and spurious proposals. The fresh QR_MAX_DIM=800 `lots` v7
artifact measured contained `[0, 5, 58, 424]` and spurious `[0, 1, 33, 10]`
bands from 487/44 proposals. High-score spurious proposals are common, so a
global score-threshold increase is not justified; any next detector change
must preserve this diagnostic and controlled dense retention. This is a
diagnostic-only follow-up, not a routing or acceptance change.

**2026-07-13 deterministic contour ordering:** connected-component boxes are
now returned in raster order before the contour family's order-sensitive
nearby merge. This removes hash-map iteration variance from bounded contour
proposals without changing thresholds, scan budgets, or the appended-frontier
cap. Two consecutive QR_MAX_DIM=800 `lots` runs retain the same non-timing
metrics; the recorded artifact reports 104/420 finder and 91/420 grouping
recall, 489/45 contained/spurious proposals, and 187/116 raw/appended contour
observations. Nominal finder recall remains 71/78. This makes the evidence
reproducible and modestly improves the prior recorded dense stage result; it
does not complete WP-010, whose broad proposal gap remains.

**2026-07-16 empty-cell ROI rejection:** On the first QR_MAX_DIM=800 `lots`
image, the retained candidate-populated ROI policy measured 34/60 finder and
29/60 grouping recall (143 contained and 5 spurious proposals). A bounded
variant reserved four of the same twelve total 192px windows for highest-
transition cells with no primary candidate, leaving eight populated windows.
It measured the identical 34/60 finder and 29/60 grouping recall and identical
contained/spurious proposal counts, while reducing raw observations 434→417.
Because it produced no one-image recall improvement, the source change was
reverted; the existing twelve populated-cell windows and downstream frontier
remain unchanged. This is negative experiment evidence, not completion of
WP-010.

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

**2026-07-13 bright-spot/glare geometry audit:** Both one-image category
artifacts reach transform and then fail format decoding, with no false
positives: bright_spots is 0/3 in 393.66 ms (23 decode attempts) and glare is
0/1 with a late discarded result in 2,607.54 ms (39 attempts). Neither route
uses saturation masking or confidence-guided RS erasures. A temporary bounded
1.25x sample-footprint retry was eligible only for small-module/high-version
candidates and made zero attempts on both exact probes, so it was reverted
rather than retained as dormant recovery work. The repeat probes stayed at
0/3 and 0/1 with zero false positives. A category-level improvement needs
sampling evidence that reaches the actual format-fail candidates, not a global
footprint retry.

**2026-07-13 tighter-footprint rejection:** A second decoder-only experiment
ran one 0.72x footprint retry after a normal format-path miss. It was fully
reverted: bright_spots remained 0/3 with zero false positives while attempts
rose from 23 to 94 (393.66 ms baseline core; 756.34 ms repeat end-to-end), and
glare remained 0/1 with zero false positives (52 attempts; 1,918.88 ms
end-to-end). Neither probe produced a successful scale retry. This excludes a
global tighter sampling retry as a bounded category fix.

**2026-07-13 bright-spots format audit:** The exact `bright_spots/image001`
probe at `QR_MAX_DIM=800` and a 2,500 ms cooperative deadline remains 0/3
with zero false positives/duplicates. Binarization, finder detection,
grouping, and transform construction all succeed, but the old `format-fail`
classification could not prove a sampled-format failure: `format_extracted`
was not populated by the decoder, so it was zero for every no-decode request.
The retained request-scoped telemetry now records observed BCH-valid format
candidates and their 0--3 distance histogram, plus RS candidate/block
attempts, successes, and uncorrectable blocks; a new matched probe must use
that evidence before naming a downstream failure. A
decoder-only candidate-targeted trial moved the existing four nearest soft-BCH
format hypotheses (distance 4--6) ahead of recovery, only after an exact
format miss and only for recovery-eligible candidates. It remained 0/3 with
zero false positives/duplicates and 23 candidate attempts; it was reverted.
The decoder already considers those soft candidates and then all 32 EC/mask
pairs in bounded recovery. Future work must use the truthful per-candidate
format/BCH and RS evidence, then retain a change only with matched recall gain;
the transient artifacts are `/tmp/wp011_format_baseline.json` and
`/tmp/wp011_format_early_soft.json`.

**2026-07-13 truthful decoder traces:** Fresh `QR_MAX_DIM=800`, 2,500 ms
diagnostics now distinguish format and RS evidence. `bright_spots/image001`
remained 0/3 with no false positives, but reached 48 strict-BCH-valid format
candidates (all distance zero) and zero RS candidate/block attempts; its
immediate barrier is therefore after format validation but before RS payload
work. `glare/image001` reached transform construction but no strict-BCH or RS
evidence before its cooperative timeout; `high_version/image001` had no
finders and likewise no decoder evidence. The three tracked artifacts are
`artifacts/wp011_category_{bright_spots,glare,high_version_image001}_trace_`
`qrmax800_limit1_2500.json`. Glare attempt counts vary under the cooperative
deadline, so the artifact is control-flow evidence rather than a latency
comparison. These traces define separate next investigations; they do not
prove a common recovery mechanism.

**2026-07-13 bright-spots post-format gate:** A fresh matched release trace
at `QR_MAX_DIM=800`, limit 1, and a 2,500 ms cooperative deadline retained
the 0/3 result with zero false positives, duplicates, or timeouts. New
request-scoped decoder evidence recorded 48 strict-BCH candidates (all
distance zero), zero RS candidate/block attempts, and 7,056
`nonzero_remainder_bit_rejections`. This is the ISO remainder-bit check after
unmasking and before codeword/RS work; it includes the strict and bounded
recovery payload hypotheses, so it is intentionally larger than the 48
strict-BCH observations. The failure is therefore sampled-module corruption
at that exact structural gate, not an RS-correction failure. The retained
artifact is
`artifacts/wp011_category_bright_spots_remainder_trace_qrmax800_limit1_2500.json`.
No recovery heuristic was added.

**2026-07-13 clean scheduler stage traces:** On committed revision `135d311`,
fresh one-image release probes at `QR_MAX_DIM=800` and a 2,500 ms cooperative
deadline show that glare and high-version are not detector-only failures. Both
reach finder, grouping, and transform stages (each counter is 1), but reach no
strict-BCH format candidate, non-zero-remainder rejection, RS candidate/block,
or RS-erasure evidence (all zero). `glare/image001` remains 0/1 with 51 decode
attempts and 1,125 high-version subpixel attempts; it completed before the
cooperative deadline. The selected `high_version/image000` remains 0/1 with a
late result discarded by the deadline, 11 decode attempts, 420 subpixel
attempts, and six refinement attempts with zero successes. These are distinct
pre-decoder sampling/geometry boundaries, while bright spots remains the
separate post-format remainder-bit boundary. The retained artifacts are
`artifacts/wp011_category_glare_stage_trace_135d311.json` and
`artifacts/wp011_category_high_version_stage_trace_135d311.json`; they are
control-flow evidence, not latency or category-improvement claims.

**2026-07-13 timing-translation rejection:** A bounded geometry-only trial
applied the existing nine-point gray timing-line translation search once to
high-version candidate transforms before sampling. It added no format/matrix
recovery hypotheses and was fully reverted. On the same one-image probes it
left glare at 0/1 with no format or RS evidence and changed the cooperative
run from 51 to 54 attempts (2,769.63 ms end-to-end); high_version remained
0/1 with no format or RS evidence and changed from 11 to 12 attempts
(3,842.36 ms end-to-end). The clean monitor smoke remained 1/1, but neither
target made decoding progress and both target runs were slower, so this
generic translation pass is not retained. Transient artifacts are
`/tmp/wp011_{glare,high_version,monitor}_gray_timing_experiment.json`.

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

**2026-07-13 high-version deadline probe:** A one-image `high_version` run at
`QR_MAX_DIM=800` with `--timeout-ms 2500` scored 0/1 and one timeout, but took
11,680.04 ms end-to-end (11,630.31 ms core). Its 29 decode attempts included
2,925 subpixel samples and 48 bounded refinement attempts, with no refinement
success. The request deadline is cooperative. The outer scheduler now checks
it before every primary binarization pass, after binarization before finder
scans, before contour fallback scans, and before ROI normalization;
cancellation uses the same gate. A deterministic zero-deadline unit test
proves no binarization or finder pass starts for an already-expired request.
The scan APIs themselves cannot yet be interrupted, so a late in-flight
operation can still exceed the budget and its result is discarded. This closes
the extra-pass scheduling gap, not the hard wall-clock-cap gap; see
`docs/wp011_high_version_probe.md`. Preserve the in-progress status until
category-level before/after artifacts are collected.

**2026-07-13 scheduler recheck:** The same `high_version --limit 1` command
still scored 0/1 with one timeout, recording 3,406.17 ms core and 3,455.38 ms
end-to-end. It made 11 decode attempts and scheduled no later primary-policy
fallbacks. This is a local control-flow recheck only, not a comparable
performance claim while WP-014 changes share the worktree; the full artifact
is `/tmp/wp011_high_version_scheduler_1_2500.json` and details are in
`docs/wp011_high_version_probe.md`.

**2026-07-16 timing-gate observation:** The fixed 0.60 horizontal-and-vertical
timing alternation gate is now observable without changing its decision.
Release one-image probes at `QR_MAX_DIM=800` and a 2,500 ms cooperative
deadline remained 0/1 with zero BCH or RS evidence: glare recorded 1,545
timing-gate rejections with mean rejected horizontal/vertical ratios of
0.107/0.141; high-version recorded 433 rejections with means of 0.519/0.502
and one cooperative timeout (3,420.92 ms core). The focused orientation test
and `cargo clippy --all-targets --all-features -- -D warnings` pass. The
artifacts are `artifacts/wp011_glare_timing_gate_2026-07-16.json` and
`artifacts/wp011_high_version_timing_gate_2026-07-16.json`. These aggregate
means are below the gate on both axes, especially for glare, so they do not
justify relaxing the threshold; the next geometry work needs a sampling or
transform change that measurably improves timing evidence first.

**2026-07-16 rejected endpoint-phase probe:** A reversible high-version-only
probe kept the three finder corners fixed and ranked a 3x3, plus-or-minus
0.25-module bottom-right endpoint phase grid by raw grayscale timing contrast.
It could replace at most one failed timing-gate sample, required a 3% score
gain, and checked the request deadline before every score and replacement;
the 0.60 gate and decode frontier were unchanged. The matched
`QR_MAX_DIM=800`/`high_version --limit 1`/2,500 ms release run remained 0/1
with one timeout, 433 timing-gate rejections, unchanged horizontal/vertical
ratio sums of 224.934570/217.524475, and zero BCH or RS evidence. The
eligible central high-version endpoint path did not occur on this route, so
the selected replacement never ran; the code was reverted rather than left
dormant. The paired `nominal --limit 1` control had one hit, zero false
positives, duplicates, and timeouts. The transient artifacts are
`/tmp/wp011_{hv,nominal}_brphase_trial.json`; a future geometry experiment
must first prove that it reaches an actually sampled high-version hypothesis.

**2026-07-13 bounded category audit:** Seven one-image local diagnostics at
`QR_MAX_DIM=800` with `--timeout-ms 2500` are retained as
`artifacts/wp011_category_*_qrmax800_limit1_2500.json`. The exact results are:
perspective 0/1 in 1919.41 ms; rotations 2/3 in 267.28 ms; brightness 2/3 in
225.88 ms; bright_spots 0/3 in 613.33 ms; glare 0/1 with one cooperative
timeout in 2790.12 ms; shadows 2/3 in 598.10 ms; and curved 0/1 in 1407.06
ms. Every artifact has zero false positives and zero duplicates. This is a
scope-bounded current-state diagnostic, not category acceptance or a
before/after claim: it samples only the first image per category, perspective,
bright_spots, glare, and curved still have no match, and glare demonstrates
that an in-flight operation can outlast the cooperative deadline. See
`docs/wp011_category_probe.md` for raw-artifact names, core timings, and
attempt counts.

**Acceptance criteria:**

- Improvements are demonstrated separately for `high_version`, `perspective`,
  `curved`, `rotations`, `brightness`, `bright_spots`, `glare`, and `shadows`.
- Candidate refinements are ranked and bounded by deadline.
- Clean nominal images do not pay the full recovery cost.

---

## WP-012: Dense multi-QR detection

**Status:** In progress; controlled 50-code gate complete, broader dense-scene
evidence remains open

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

**2026-07-13 bounded dense-routing follow-up:** The scanline finder cap now
permits a finite 64 independent markers per row/column (rather than silently
stopping after five), dense inputs can enter the bounded spatial path up to
384 finder proposals, and its ranked/group frontier remains capped at 128 by
the request candidate budget. Local anchor-best grouping seeds (16 nearby
same-scale proposals per anchor) raise the controlled 50-symbol scene from
the prior 2/50 (4.00%) baseline to 29/50 (58.00%) with zero false positives
or duplicates. Finder recall is now 50/50, but grouping/decode remains 29/50
and takes 2323.97 ms, so the 90%/<500 ms target remains unmet. This is a
bounded routing improvement, not completion; the remaining failure is
spurious local finder evidence preventing all true triples from being routed.

**2026-07-13 bounded region-cap follow-up:** The controlled 50-symbol scene
had 50 retained local groups, but the `MultiQrHeavy` router admitted only 32
regions. The router now visits one region for every retained dense candidate,
while retaining the existing 128-candidate frontier and request-wide decode
cap. The regenerated public `qrtool reading-rate` artifact reports 43/50
(86.00%) at 2139.27 ms, with zero false positives and zero duplicates; the
full 1–100 corpus is 145/193 (75.13%). This is a measured improvement over
the earlier 32/50 run, not completion: the 90%/<500 ms target remains unmet.
See `docs/wp012_raster_scenes.md` and
`artifacts/wp012_controlled_dense_by_density_qrmax0.json`.

**2026-07-13 bounded strict-transform scheduling:** The first two dense
recovery-eligible candidates preserve the existing 25-point bottom-right
transform sweep. Every later retained candidate still receives a strict ISO
decode, but uses a fixed center-plus-cardinal five-point sweep instead of the
full recovery frontier. A three-iteration in-process release probe retained
48 decoded symbols per call and reduced warm `dense_50` time from 1779.29 ms
to 414.72 ms; requested bytes fell from 94,802,508 to 36,307,489. The public
geometry evaluator measured 43/50 (86.00%) in 455.85 ms with zero false
positives, zero duplicates, and no timeouts. This clears the controlled
latency half of the target, not the 90% recall half, so WP-012 remains in
progress.

**2026-07-13 post-scheduling miss diagnosis:** A fresh public evaluator run
was 43/50 in 485.14 ms with zero false positives, duplicates, or timeouts.
The missing payloads are `WP012-011`, `-014`, `-021`, `-039`, `-043`, `-046`,
and `-047`; finder evidence remains 50/50. The bounded telemetry evaluator
selects the brightness route for this white-background raster, and that route
returns 43. The direct fast Otsu API returns 48 valid payloads but takes about
690 ms, so it cannot be substituted under the 500 ms acceptance budget. No
matrix-validation, RS-recovery, or request-cap relaxation is justified. The
next task is per-candidate brightness-route diagnostics followed only by a
time-capped strict sampling/geometry recovery that proves >=45/50, FP=0,
duplicates=0, and <500 ms in the public evaluator. See
`docs/wp012_raster_scenes.md`.

**2026-07-13 rebuilt route classification (`fed7d19`):** A fresh
`dense-route-audit` uses the same public 10,000ms deadline for brightness and
compares it with direct strict Otsu on `density_050`. Brightness yields 43;
Otsu yields 48; every brightness geometry is shared. The Otsu-only geometries
are `WP012-011`, `-014`, `-039`, `-043`, and `-046`; `-021` and `-047` are
absent from both. The audit records 161 Otsu finders and the bounded 128-group
frontier with raw payload/bbox evidence. A fresh public evaluator is still
43/50 in 394.62ms with FP=0, duplicates=0, and no timeout. Therefore no route
policy changed: the remaining safe task is to select only the five Otsu-only
candidate regions without a full second decode.

**2026-07-13 direct-Otsu cost audit (`fed7d19`):** The same rebuilt route
artifact measures direct Otsu at 11.477ms binarization, 37.265ms finder,
18.642ms diagnostic grouping, and 293.847ms decode (361.231ms including the
separate grouping observation). Brightness alone took 348.653ms. The public
394.62ms dense_50 lane has about 105ms before the 500ms gate, so a complete
second Otsu route cannot fit. Only a non-duplicate preselection of the five
known Otsu-only geometries with a strictly small decode frontier is technically
plausible; no behavior was changed by this diagnostic.

**2026-07-13 rejected residual-Otsu probe:** A local generic experiment held
back eight of the 128 request attempts, filtered direct-Otsu finder evidence
inside the 43 accepted brightness regions, and decoded only the residual
frontier. It was reverted: the public evaluator regressed to 40/50 in 911.42
ms (FP=0, duplicates=0, timeouts=0). Reserving attempts lost primary-route
symbols while the residual candidates still exceeded the latency budget. Do
not repeat this filter-plus-fixed-budget design; any future hybrid needs a
cheaper candidate-level selector.

**2026-07-16 rejected bounded Otsu geometry anti-join:** A reversible dense
post-primary probe retained the gamma route and its request budget, then used
only plain-Otsu binarization/finder/grouping to rank at most five
spatially-distinct residual candidates for center-plus-cardinal strict decode.
It did not improve the current 48/50 public `density_050` result. The two
remaining payloads (`WP012-021`, `-047`) are absent from the current Otsu
frontier too; an initial bbox-IoU anti-join selected five already-decoded or
cross-symbol triples and still returned 48/50, while the corrected
finder-center containment plus dense-scale filter selected no residual
candidates. The latter isolated public run was 48/50 (96.00%) in 168.39 ms,
with FP=0, duplicates=0, and timeouts=0. The selector code was reverted. Do
not retry an Otsu-only residual selector without evidence that a remaining
miss has a distinct Otsu finder/group candidate.

**2026-07-16 rejected breadth-first threshold-20 probe:** A scheduler-only
experiment lowered the `MultiQrHeavy` breadth-first region threshold from 40
to 20, without changing the 128-group frontier, candidate/transform caps,
lane budgets, grouping, or decode acceptance. The focused pipeline tests
passed, and a rebuilt release CLI ran the full controlled ladder. It did not
recover the two 25-symbol misses: the results were 1/1 in 3.04 ms, 2/2 in
4.96 ms, 5/5 in 192.05 ms, 10/10 in 349.24 ms, 23/25 in 53.96 ms, 48/50 in
93.84 ms, and 63/100 in 193.68 ms; every point had zero false positives,
duplicates, and timeouts. Since the required 24/25 density-25 improvement did
not materialize, the threshold and its unit test were reverted. Do not retry
this threshold-only schedule adjustment; future work needs evidence that the
public density-25 route reaches a distinct schedulable candidate for either
remaining label.

**2026-07-13 release-binary correction:** A transient 27/50, 4.8-second
`density_050` result came from stale `target/release/qrtool` bytes, not a
source regression. After `cargo build --release --features tools --bin qrtool`,
the identical isolated evaluator command at current `bbeefb9` returned 43/50
in 364.4408 ms with zero false positives, duplicates, and timeouts; see
`artifacts/wp012_dense50_rebuild_check.json`. Rebuild the release CLI before
every evaluator run. This invalidates the stale artifact only and does not
change WP-012's still-unmet 90% recall criterion.

**2026-07-13 release-build reproducibility check:** A transient 27/50,
4.8-second result was traced to a stale `target/release/qrtool` binary and is
invalid as source evidence. After `cargo build --release --features tools --bin
qrtool`, the direct `density_050` check again returned 43/50 in 364.4408 ms
with zero false positives, duplicates, or timeouts; the raw observation is
`artifacts/wp012_dense50_rebuild_check.json`. Rebuild the release CLI before
comparing any WP-012 artifact to source changes.

**2026-07-13 dense region scheduling:** When the multi-QR router has at least
40 disjoint retained regions, it now spends the existing bounded decode budget
round-robin across regions before taking a second candidate from any region.
This prevents one cluster's near-duplicate triples from starving a distinct
symbol. The regenerated public raster artifact
`artifacts/wp012_controlled_dense_round_robin_qrmax0.json` records 48/50
(96.00%) in 97.09 ms at density 50, with zero false positives, duplicates, or
timeouts; three isolated reruns were also 48/50 in 95.84–101.91 ms. This
meets the controlled 50-code acceptance target. WP-012 remains in progress:
the same density ladder has 23/25 and 63/100, so the broader dense-scene goal
and realistic-scene evidence are still open.

**2026-07-13 realistic-lots boundary check:** The retained raster scheduling
change does not establish photographic multi-code readiness. At `QR_MAX_DIM=800`,
`reading-rate --category lots --limit 1 --timeout-ms 2500` localized 1/60
codes in 505.77 ms with zero false positives, duplicates, or timeouts; the
transient artifact is `/tmp/wp012_lots_round_robin_limit1.json`. This keeps
WP-010 proposal recall and realistic-scene localization as the next blockers.

**2026-07-13 realistic-lots route rejection:** On `lots/image001`, stage
telemetry reaches 34/60 contained finder proposals and 29/60 grouped symbols,
but the public fast/brightness route returns only 1/60. Bypassing its dense
early returns was tested without changing candidate caps: the full bypass
stayed at 1/60 in 1.16 s, while a sparse-return variant reached 4/60 in 1.02 s.
Both exceed the 500 ms controlled dense latency target and do not approach
acceptable recall, so they were reverted. The next route must
select/decode the existing grouped candidates under the bounded budget; do not
disable these early returns globally.

**2026-07-13 bounded adaptive-frontier rejection:** The brightness telemetry
path reused only attempts left after its dense one-code gamma decode for one
adaptive grouped frontier and merged only distinct payloads. It left realistic
`lots` at 1/60 while raising wall time from 497.8 to 922.5 ms; controlled
`dense_50` held 48/50 in 94.5 ms. The frontier neither selected a useful real
candidate nor met the latency bound, so it was reverted. Do not repeat this
post-primary adaptive-frontier design.

**2026-07-16 post-group strategy rejection:** `lots/image001` had 90–91
ranked candidates, 13 regions, and 37 bounded attempts. Forcing
`MultiQrHeavy` once the frontier reached 12 candidates did not change its
public result (still 1/60 at about 494 ms). The existing grouped candidates do
not presently yield more successful decodes; broader routing alone is not the
missing step. The temporary selector was reverted. Do not repeat a
candidate-count-only `MultiQrHeavy` promotion.

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
no decode for that image. A fresh six-adapter smoke artifact
(`artifacts/competitors/wp013-local-smoke-2026-07-13.json`) records the exact
missing pinned build outputs: `ZXingReader`, `quirc_decode`,
`boofcv_decode.jar`, and `rqrr_decode` under `competitors/bin/`. The harness
records each adapter's setup state and passes an explicit one-thread
OpenMP/BLAS environment to every child process. Passed:

```bash
python3 -m unittest scripts/tests/test_run_competitor_harness.py
python3 scripts/run_competitor_harness.py --category monitor --limit 1 \
  --adapter zbar --adapter opencv --timeout-ms 5000 \
  --output /tmp/competitor-smoke.json
git diff --check
```

**2026-07-13 local availability preflight:**
`rustqr.competitor-preflight.v1` now records no-decode local runner/version/
capability state before image materialization. The fresh artifact
`artifacts/competitors/wp013-local-preflight-2026-07-13.json` observed ZBar
0.23.93 with `zbarimg --raw` and OpenCV 4.12.0 with `QRCodeDetector` plus
`detectAndDecodeMulti`. Both states are deliberately
`available_version_matches_unverified_provenance`: they confirm the locally
available protocol surface but cannot prove a pinned source revision, build
flags, wheel origin, or comparator eligibility. This advances reproducible
setup diagnostics only; it creates no pinned-build, accuracy, latency, or
competitor-comparison claim.

**2026-07-13 local pinned-runner verification:** The resolved Homebrew ZBar
0.23.93 executable is now locked by SHA-256 in `competitors/lock.json` and
`adapter_runner_path()` resolves PATH commands before hashing them. Fresh
preflight evidence in
`artifacts/competitors/wp013-zbar-pinned-preflight-2026-07-13.json` reports
`verified_pinned_runner`; a one-image monitor smoke in
`artifacts/competitors/wp013-zbar-pinned-smoke-2026-07-13.json` exercised the
same byte-identical PGM contract offline (one no-decode, so no accuracy or
comparison claim). The current local Cargo registry cannot resolve
`rqrr = 0.9.0` in `--offline` mode, so the Rust adapter still requires its
pinned crate source/index before it can be built; ZXing-C++, quirc, and BoofCV
sources/runners are likewise absent locally.

**2026-07-13 verified ZBar monitor observation:** The harness now records each
adapter's runner preflight with a normal report and has a
`--require-verified-runner` gate. Non-invoked unavailable, version-mismatched,
or provenance-unverified adapters are excluded from timing and
annotation-count coverage rather than misreported as zero-return decodes.
`artifacts/competitors/wp013-zbar-monitor-2026-07-13.json` used that gate for
all 17 `monitor` images at the shared 1024px BT.601/triangle-PGM contract.
ZBar 0.23.93 was `verified_pinned_runner`, returned one payload line
(`monitor/image008.jpg`), and had 16 no-decodes with a 107.67ms median and
133.83ms p95 process-invocation time. The 1/17 returned-line/annotation count
is deliberately not payload accuracy, localization recall, or a RustQR/ZBar
comparison: the labels do not contain payload truth and the ZBar CLI exposes
no quadrilaterals. This is retained reproducible adapter evidence only.

**2026-07-13 verified quircs smoke:** The locally cached `quircs` 0.10.3 crate
and every Cargo-lock dependency build offline with `--locked`. Its dedicated
P5-only adapter is SHA-256 pinned in `competitors/lock.json`; the preflight
artifact `artifacts/competitors/wp013-quircs-pinned-preflight-2026-07-13.json`
reports `verified_pinned_runner`. The `--require-verified-runner` monitor
smoke at `artifacts/competitors/wp013-quircs-pinned-smoke-2026-07-13.json`
returned one raw payload line for the one annotated image in 9.61ms. This is
only a runnable-adapter/count observation: the monitor labels do not provide
payload truth or quadrilaterals from the competitor, so it is not accuracy,
localization, or a RustQR comparison claim.

**2026-07-13 verified payload-only subset:**
`artifacts/competitors/wp013-payload-conformance-2026-07-13.json` uses six
fixed ASCII-safe generated conformance matrices with exact raw-payload truth,
source hashes, and shared materialized-PGM hashes. Both SHA-pinned runners
(ZBar 0.23.93 and quircs 0.10.3) returned the one expected payload line for
all six cases. This is reproducible payload-only competitor evidence, not a
localization evidence: neither competitor CLI reports quadrilaterals. At that
point RustQR was not yet an adapter on the shared-PGM timing boundary; the
following record supersedes that limitation for payload-only fixtures.

**2026-07-13 RustQR shared-pixel adapter:** A source-controlled P5 adapter now
uses RustQR's public `ImageInput`/`try_detect_with_options` API. The same six
fixture PGM bytes produced exact payload matches from SHA-pinned RustQR 0.1.0,
ZBar 0.23.93, and quircs 0.10.3 runners. This remains deterministic
payload-only evidence, with no geometry or end-to-end latency claim.

**2026-07-13 shared-PGM monitor diagnostic:** The fixed 17-image monitor lane
now has a single artifact for the verified RustQR, ZBar, and quircs runners.
At the common 1000ms external process timeout RustQR decoded 2 and timed out
on 15; ZBar decoded 1/no-decoded 16; quircs decoded 16/no-decoded 1. These are
explicit runner observations only. The monitor labels lack payload truth and
all adapters lack geometry output, so neither returned-line counts nor
process-only timing is a correctness, recall, or fair-latency claim.

**2026-07-16 synthetic shared-PGM geometry-v1:** The SHA-pinned RustQR,
ZBar, and quircs runners now emit/parse payload-plus-corner records for the
same six delimiter-safe ASCII fixtures. The verifier derives each reference
quadrilateral from the generated symbol dimension, 4-module quiet zone, and
4 pixels/module; it canonicalizes winding/start corner before calculating
convex-polygon IoU, and requires one exact payload record plus IoU >= 0.99.
It deliberately preserves native raw corners rather than snapping them to the
generator grid. All 18 payload checks are exact and quircs has 1.0 IoU on all
six, but the strict gate is currently **non-passing**: ZBar is 0.980100 on
`v02-M-m0-byte` and 0.983395 on `v07-Q-m0-byte`; RustQR is 0.979994 on
`v07-Q-m0-byte`. The checked-in artifact contains every raw record and IoU.
This is synthetic shared-PGM geometry evidence only, not real-scene
localization recall or a fair latency comparison. Do not soften the threshold
or alter raw corners to make this gate pass; any recovery needs an independently
justified geometry convention or actual detector improvement.

**2026-07-16 rejected alignment tie-break:** The v7 RustQR miss is not an
adapter convention issue: an exact-match 4x4 alignment-pixel plateau lets the
row-major search select `(168,168)` although the predicted center is
`(170,170)`, contracting the returned bottom-right corner. A narrow equal-
mismatch tie-break that preferred the nearest predicted center was tested with
focused geometry tests and the shared-PGM gate. It left v7 RustQR at
0.979994 IoU but regressed the previously correct v2 RustQR fixture from
0.99999999 to 0.9563219. The change was fully reverted. Do not replace the
existing deterministic raster tie-break with predicted-center proximity; any
future refinement must explain both symbols and preserve the raw-corner gate.

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

**2026-07-13 dense recovery evidence:** The successful controlled `dense_50`
lane showed that candidates beyond the existing two-attempt recovery budget
were still running matrix-level non-canonical recovery after a strict decode
miss. The decoder now preserves the strict ISO decode for every candidate and
gates only fallback traversal/format and confidence-beam repair by that same
bounded flag. A three-iteration local release probe retained 48/50 results per
call, reduced allocations from 451,087 to 444,857 and requested bytes from
95,437,204 to 94,802,508. Warm time changed from 1.725 s to 1.732 s, so no
latency improvement is claimed. See `docs/wp014_baseline.md` for the exact
command and raw probe boundaries.

**2026-07-13 1024-pixel credibility sample:** A bounded five-process-run,
five-warm-call collection at `QR_MAX_DIM=1024` is recorded in
`artifacts/wp014_credibility_1024_a60d082.json`. Clean returned one result in
every run (process p50 54.50 ms, p95 266.26 ms; warm detect-only average
37.46 ms), so it does not meet the 100 ms p95 bound. The required hard lane
returned zero results in all five runs (process p50 2516.36 ms, p95 2717.33
ms) and its allocation profile correctly aborts rather than producing a
success timing. The successful controlled `dense_50` route was Triangle
resized from 1276x1119 to 1024x898 and returned 48 direct results in every
run (process p50 354.12 ms, p95 355.46 ms; warm detect-only average 369.69
ms), so it misses both near-term latency bounds. The allocation harness emits
one five-call aggregate, not per-call samples, therefore cannot establish an
in-process p50/p95. This is evidence of non-passage, not a target claim; the
full boundary and raw observations are in `docs/wp014_baseline.md`.

**2026-07-13 scanline allocation reduction:** Finder scanlines previously
allocated two growing run-history vectors per scanned row or column even
though validation reads only the latest five runs. The fixed five-entry stack
window preserves that suffix for primary and bounded ROI scans. A three-warm
call release probe on the successful clean lane retained one decoded symbol
and reduced first-call allocations from 69,411 to 66,119, reallocations from
5,468 to 1,525, and requested bytes from 4,656,124 to 4,249,900. The isolated
42.681 ms to 42.526 ms warm-time difference is not a latency claim. Full
all-feature tests and ignored release photographic regressions passed. See
`docs/wp014_baseline.md` for the reproducible command and timing boundary.

**2026-07-13 rank-frontier copy removal:** `rank_groups` now iterates the
existing capped raw-group frontier directly instead of first allocating and
copying `groups_to_process`. Ordering, the dense/normal caps, and all group
contents are unchanged. One-call release allocation probes retained the clean
1-symbol and controlled dense_50 48-symbol results; dense_50 recorded 285,750
allocations and 33,155,609 requested bytes. This is allocation evidence only;
the 86.13 ms dense and 42.76 ms clean samples are not latency claims. Passed
pipeline grouping tests and `cargo test --all-features`.

**2026-07-16 region-frontier capacity reservation:** `cluster_regions` now
reserves only its existing `min(max_regions, candidate_count)` frontier before
appending region clusters. Attachment traversal, centroid updates, sorting,
and the normal/dense region caps remain unchanged. Matched one-warm-call
release dense_50 probes retained 48 decoded symbols and 285,790 allocations,
while reducing reallocations from 5,928 to 5,924 and requested bytes from
33,156,089 to 33,155,961. This is allocation evidence only; the runs' timing
samples are not a latency claim. Focused spatial-grouping tests and the full
controlled-density ladder passed (including dense_50: 48/50, no timeout).

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

**2026-07-13 product-readiness documentation:** Added
`docs/product_readiness.md`, grounding the current `0.1.0` experimental
versioning boundary, Rust 1.85 MSRV evidence, no-claim security intake limits,
BoofCV-versus-self-authored-corpus provenance boundary, and unsupported
platform/binding status in tracked repository state. This is documentation
completion only; crates.io publication, tags, a response-time commitment,
license verification for external datasets, and binding releases remain open.

**2026-07-13 MSRV evidence:** on the installed `rustc 1.85.0` toolchain,
`cargo +1.85 test --lib --no-default-features` and `cargo +1.85 test --lib`
each passed 112 library tests on macOS AArch64 before the later scheduler and
corpus work. After that work stabilized, `cargo +1.85 test --all-features`
passed on macOS AArch64: 133 library tests, 5 CLI tests, 3 adversarial tests,
7 regular conformance tests (with the 3,840-case full-grid test intentionally
ignored), 4 mutation tests, 7 input API tests, 6 synthetic negative-corpus
tests, and 7 intentionally ignored photographic regressions. The MSRV CI lane
now runs that exact all-features command. This verifies the supported desktop
feature matrix, not WASM, iOS, Android, bindings, or `no_std`.

**2026-07-16 matrix-core seam:** Added the public alloc-owned
`MatrixDecodeResult` and allocation-free `MatrixRecoveryBudget` types. The
deterministic matrix decoder and payload path now return the matrix-only result;
the hosted `QrDecoder` converts it to `QRCode` only where image-specific
position, sampled modules, and confidence are needed. The generated supported
corpus checks exact raw bytes, text, version, EC level, mask, and metadata
parity between `decode_matrix_result` and the existing `decode_matrix_for_mode`
API. `cargo test --lib --all-features` (142 tests), `cargo test --test
conformance_matrix_tests --all-features` (7 passed; 3,840-fixture grid still
ignored), `cargo test --lib --no-default-features` (122 tests), and `cargo
clippy --lib --all-features -- -D warnings` passed. This is an extraction seam,
not `no_std` support: `DecodeRequestContext` still owns hosted deadline and
cancellation controls, uncertain-module repair still reads `Instant`, and no
separate core crate or target lane exists. The next required slice is to move
the matrix result/budget plus decoder modules into a separately built crate and
run the same fixtures there before adding any `no_std + alloc` target claim.

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

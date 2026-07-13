# ADR-0001: retain the Model 2 codebase; do not merge the rebuild

**Date:** 2026-07-12  
**Status:** decided, with a remaining benchmark-normalization gate

## Context

WP-005 requires a choice between `main`, `scratch_from_scratch_rebuild`, and
the current Model 2 work on `codex/wp006-conformance`.  The historical roadmap
also names `work_wp014_wp018_gates_2026_02_12`; that is a separate later branch,
not the rebuild branch described by the preserved benchmark evidence.  This ADR
uses the actual rebuild tip, `295b97c`.

The three implementations have incompatible benchmark contracts:

- `main` emits `rustqr.reading_rate.v1`, where BoofCV labels are reduced to a
  count-based score.
- the current branch emits `rustqr.reading_rate.v2`, which performs scaled
  quadrilateral one-to-one localization matching and separately reports false
  positives, duplicates, and timeouts.
- the rebuild emits `wp007-reading-rate-v1`; annotation labels are successful
  whenever *any* code is returned for an image, and its rate is a fraction,
  not a percentage.

The v2 comparator correctly refuses the v1 artifact, so the historical rates
must not be presented as an A/B delta.

## Evidence collected locally

Each binary was built from an isolated `git archive` export, then run against
the exact same current worktree pixels at `QR_MAX_DIM=800` (the rebuild used
its equivalent `--max-working-dim 800`).  The six ordinary categories used a
limit of three images; `lots` used one image, as prescribed by the roadmap's
small-run guidance.  The combined SHA-256 of the selected-category image list
was `37fdaff3033d604e9f616e671f1ab2e72adadd692f48eadeced56448163117ca`.

| Category | main: count score, median | current: v2 localization, median | rebuild: image score, median |
| --- | --- | --- | --- |
| nominal | 3/6 (50.00%), 1245.9 ms | 1/6 (16.67%), 895.9 ms | 0/3 (0.00 fraction), 60.5 ms |
| rotations | 0/9 (0.00%), 793.9 ms | 1/9 (11.11%), 773.6 ms | 1/3 (0.33 fraction), 176.5 ms |
| perspective | 3/3 (100.00%), 321.6 ms | 0/3 (0.00%), 711.2 ms | 3/3 (1.00 fraction), 144.1 ms |
| high_version | 0/3 (0.00%), 1271.6 ms | 0/3 (0.00%), 1241.2 ms | 0/3 (0.00 fraction), 178.8 ms |
| lots | 1/60 (1.67%), 870.7 ms | 1/60 (1.67%), 187.1 ms | 0/1 (0.00 fraction), 324.3 ms |
| brightness | 2/10 (20.00%), 334.2 ms | 1/10 (10.00%), 525.8 ms | 1/3 (0.33 fraction), 251.9 ms |
| bright_spots | 0/9 (0.00%), 243.3 ms | 0/9 (0.00%), 327.0 ms | 0/3 (0.00 fraction), 258.8 ms |

These are diagnostics only.  They demonstrate that the rebuild is executable
on the selected pixels, but they do not establish accuracy or latency wins:
the units, matching rules, and timing boundaries differ.  In particular, its
perspective 3/3 means three images with a nonempty result, not three of three
localized symbols.

`cargo test --all-features --no-fail-fast` passed in all three exports.  The
test evidence is not comparable in scope: `main` passed 72 library tests (plus
seven intentionally ignored photographic tests), the rebuild's suite focuses
on its scaffold/pipeline modules, and the current branch passed 104 library
tests plus conformance, mutation, input-API, and deadline suites.  The current
branch also has a deterministic 3,840-fixture Model 2 gate; the rebuild does
not contain an equivalent conformance corpus.

## Decision

Continue the current Model 2 implementation.  Do not merge the rebuild branch
or treat its historical Actions results as evidence of a production win.  Its
smaller source surface is achieved by replacing the in-tree decoder with the
`quircs` dependency and by omitting the current conformance and safe-input
coverage, rather than by proving an equivalent implementation is simpler.

There are no production-code transplants accepted now.  The following rebuild
commits are quarantined candidates for a later, narrowly scoped experiment:

- `cfd969d` (`proposal_ensemble`) and `2e339b5` (`multi_qr_iteration`) may
  supply algorithm ideas, but their types and acceptance contract are
  incompatible with the current public API.
- `78f85ba` and `e735ac9` may inform profile/workflow ergonomics only.  The
  current v2 evaluator remains authoritative and must not be replaced by the
  rebuild reporter.

## Required follow-up and rollback points

### Fresh normalization audit (2026-07-13)

The comparison remains blocked by an **artifact-contract** gap, rather than by
missing input pixels or an unavailable local build:

- `scratch_from_scratch_rebuild` remains at `295b97c`.  Its public
  `QrCode` type contains four `corners`, but its `qrtool reading-rate` JSON
  serializes only `matched`, `runtime_ms`, and a failure signature per case.
  It therefore cannot be rescored for localization after the fact.
- More importantly, its evaluator treats a BoofCV `SETS` annotation as
  successful whenever `report.codes` is nonempty.  It does not serialize a
  prediction quadrilateral, apply IoU, count duplicate predictions, or retain
  false positives.  The `wp007-reading-rate-v1` artifact is consequently not
  an alternate representation of `rustqr.reading_rate.v2`.
- `main` is also not a v2 producer.  A comparison needs adapter outputs for
  both historical candidates, with the current branch's evaluator used only
  as the scoring authority.

This audit inspected the checked-out source and refs, not a new benchmark run.
It intentionally does not alter the diagnostic table above or claim a new
accuracy/latency result.

### Required adapter contract

Create throwaway comparison refs from `main` and
`scratch_from_scratch_rebuild` (do not merge either adapter).  Each adapter
must add a machine-readable prediction stream with, for every input image:

```text
image identity, original width and height, working width and height,
all predicted quadrilaterals in original-image coordinates, decoded payload
when available, core elapsed time, end-to-end elapsed time, timeout status
```

The adapter must use the current image loader and resize implementation, or
delegate both implementations to one shared external runner.  It must not turn
an image-level nonempty decode into a localization hit.  The shared scorer must
use `mode=localization;quad-iou=0.5;matching=max-cardinality`, emit the
current v2 fields (including false positives, duplicates, and timeouts), and
record separate image and label fingerprints.  A schema name distinct from
`rustqr.reading_rate.v2` is acceptable only if
`scripts/compare_reading_rate_artifacts.py` accepts it after an explicit,
tested compatibility check.

### Reproducible run sequence

After the adapters exist, run the same fixed commands from clean checkouts.
Use an explicit 1024-pixel working limit and an explicit 25-image category
limit; neither `QR_SMOKE` nor an implicit environment default may be present.

```sh
unset QR_SMOKE QR_BENCH_LIMIT QR_MAX_DIM
for category in nominal rotations perspective high_version lots brightness bright_spots; do
  QR_BENCH_LIMIT=25 QR_MAX_DIM=1024 \
    cargo run --features tools --bin qrtool --release -- \
    reading-rate --non-interactive --category "$category" \
    --artifact-json "wp005_<candidate>_${category}_v2.json"
done

python3 scripts/compare_reading_rate_artifacts.py \
  --baseline wp005_main_targeted_v2.json \
  --candidate wp005_rebuild_targeted_v2.json
```

The current `qrtool` accepts one category per invocation, while Fast Benchmark
splits comma-separated categories in its shell loop.  The loop is therefore
deliberate. Preserve all seven JSON artifacts; do not pass the comma-separated
value directly to the current CLI. The comparison command is required only
after those artifacts have been merged by the proposed adapter tool; it does
not currently exist and must be tested before it becomes evidence.

The remote validation step remains necessary because the acceptance criterion
requires a matched hardware class.  Dispatch `.github/workflows/fast-benchmark.yml`
on each comparison ref with `platform=macos`, `bench_limit=25`, and
`category=nominal,rotations,perspective,high_version,lots,brightness,bright_spots`.
Preserve the uploaded `reading_rate_*.json` artifacts and the comparator output
under immutable run-ID directories before interpreting a delta.  The workflow
already fixes `QR_MAX_DIM=1024`; a non-adapter historical ref cannot satisfy
this gate because it still emits an incompatible artifact.

Only then trial one quarantined slice at a time behind a feature flag.  Each
slice needs the full conformance gate, the v2 category report, and a commit
that cleanly reverts it.  Revert immediately on a category regression,
increased timeout rate, false-positive increase, or failed conformance case.

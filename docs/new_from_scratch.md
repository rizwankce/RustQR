# New from Scratch Plan (Target: >90% Reading Rate, Median <=1s)

**Date:** 2026-02-11  
**Benchmark:** `benches/images/boofcv` (same dataset, same scoring)  
**Goal:** Keep benchmark unchanged; redesign everything else if needed.

## Target

1. Weighted global reading rate: **>=90%**
2. Median runtime: **<=1000 ms/image**
3. Stable across categories, especially:
   - `lots` (420 labels)
   - `rotations` (133 labels)
   - `high_version` (37 labels)

## Non-Goals

- Preserving current internal architecture.
- Preserving old tuning knobs.
- Chasing micro-optimizations before structural correctness.

## Guiding Principles

1. Separate detection quality from decode quality in telemetry.
2. Use bounded search with good priors, not open-ended brute force.
3. Use feedback loops: decode confidence should improve geometry, not only accept/reject it.
4. Optimize for worst leverage categories first (`lots`, `rotations`, `high_version`).

## Execution Status (Current)

- Done: `WP-001 proposal_ensemble`
- Done: `WP-002 hypothesis_search` (finder graph + bounded L-shape beam)
- Done: `WP-003 geometry_refinement` (scaffold deterministic 3-pass refinement loop)
- Done: `WP-004 decode_engine` (scaffold bounded confidence-aware candidate + retry manager)
- Done: `WP-005 multi_qr_iteration` (scaffold strongest-first bounded acceptance + payload dedupe)
- Done: `WP-006 runtime controller + budgeting`
- Done: `WP-007 benchmark harness + KPI artifact scaffold`
- Done: `WP-008 throughput optimization` (working-resolution cap + proposal hot-path cleanup)
- Done: `WP-009 semantic naming refactor`
- Done: `WP-010` (real decode backend via `quircs`, no synthetic payload runtime path)
- Done: `WP-011` (decode + multi reserve lanes under stage over-budget)
- Done: `WP-012` (reading-rate smoke profiles for monitor/nominal)
- Done: `WP-013` (strict payload-validated benchmark lane)
- Done: `WP-014` (locked full BoofCV baseline artifact from GH run `21927167412`)
- Done: `WP-015` (`benchdiff` artifact comparator)
- Done: `WP-016` (dual KPI lanes: annotation vs strict payload)
- Done: `WP-017` split into 3 slices (`rotations`, `high_version`, `lots`)
- Done: `WP-018` (workflow KPI gates + threshold enforcement)
- Done: `WP-019` (full BoofCV profile coverage + all-profiles benchmark dispatch)
- In progress: `WP-020` (decode fallback stage deadline guardrails)
- In progress: `WP-021` (runtime KPI signal split: pipeline vs end-to-end)

Current harness supports benchmark reporting via:
- `cargo run --bin qrtool -- reading-rate --limit N --artifact <path>`
- `cargo run --bin qrtool -- reading-rate --profile monitor-smoke --artifact <path>`
- `cargo run --bin qrtool -- reading-rate --profile nominal-smoke --limit N --artifact <path>`
- `cargo run --bin qrtool -- reading-rate --profile boofcv-all --artifact <path>`
- `cargo run --bin qrtool -- reading-rate --profile boofcv-rotations --artifact <path>`
- `cargo run --bin qrtool -- reading-rate --profile payload-validated --artifact <path>`
- `cargo run --bin qrtool -- reading-rate --profile boofcv-all --gate-global-rate-min <f64> --gate-rotations-rate-min <f64> --gate-high-version-rate-min <f64> --gate-median-runtime-ms-max <f64> --artifact <path>`
- `cargo run --bin qrtool -- benchdiff --base <base.json> --candidate <candidate.json> --artifact <diff.json>`
- It runs real `pipeline::detect_with_config` evaluation per image and writes schema `wp007-reading-rate-v1`.
- Runtime knobs: `--max-working-dim N`, `--emergency-cutoff-ms N`.
- Profiles now cover all BoofCV categories (`boofcv-*`) plus `payload-validated`.
- Benchmark workflow supports `profile=all-profiles` to run every lane in one dispatch.
- Benchmark workflow also supports `profile=dual-kpi` to run official KPI lanes only (`boofcv-all` + `payload-validated`) in one dispatch.
- Benchmark workflow now supports configurable KPI gate thresholds (`gate_global_rate_min`, `gate_rotations_rate_min`, `gate_high_version_rate_min`, `gate_median_runtime_ms_max`) and fails when configured thresholds are violated.
- Label semantics:
- `boofcv/*` uses annotation labels (any decode counts as matched).
- `custom/decoding` uses strict expected payload labels (exact payload match required).

Current sample smoke baselines (2026-02-11):
- `monitor-smoke`: rate `1.0000` (17/17), median `3905.864 ms`
- `nominal-smoke --limit 20`: rate `0.5000` (10/20), median `667.834 ms`
- `payload-validated`: rate `1.0000` (26/26), median `149.876 ms`

Locked full BoofCV baseline (2026-02-12):
- Source run: GitHub Actions `Benchmark` run `21927167412` (`profile=all-profiles`, branch `scratch_from_scratch_rebuild`)
- Locked artifact: `benchmark/baselines/gh_run_21927167412/reading_rate_boofcv-all.json`
- Summary: `223/536` matched (`0.4160`), median `1908.431 ms`, top failure `over-budget`

## Proposed Architecture

Canonical implementation naming (source-of-truth):

- Stage A -> `proposal_ensemble`
- Stage B -> `hypothesis_search`
- Stage C -> `geometry_refinement`
- Stage D -> `decode_engine`
- Stage E -> `multi_qr_iteration`

## Proposal Ensemble (Stage A, `proposal_ensemble`)

- Build 3-4 binary views in parallel:
  - Otsu/global
  - Adaptive
  - Sauvola/contrast-aware
  - Optional glare-suppressed view
- Generate finder/quad proposals from all views.
- Union proposals with score normalization.

**Why:** maximize recall early with bounded extra cost.

## Hypothesis Search (Stage B, `hypothesis_search`)

- Build a finder graph:
  - Nodes: finder candidates
  - Edges: plausible scale/distance/orientation pairs
- Generate L-shape hypotheses from local neighborhoods (`k` nearest plausible nodes).
- Score with cheap invariants:
  - right-angle residual
  - module-size agreement
  - timing-line alternation
  - local contrast consistency
- Keep bounded top `H` hypotheses (beam/heap), e.g. 64.

**Why:** removes early order bias and combinatorial explosion.

## Geometry Refinement (Stage C, `geometry_refinement`)

- For each hypothesis:
  - Initialize homography
  - Refine corners/alignment with subpixel sampling
  - Fit against timing-line phase consistency
  - Re-estimate module pitch
- Run 2-3 lightweight refinement iterations.

**Why:** directly attacks current dominant `format-fail`.

## Decode Engine (Stage D, `decode_engine`)

- For each refined grid:
  - Sample with subpixel interpolation
  - Produce soft module confidences
  - Decode with BCH/RS using confidence-aware retries
- Limit retries by confidence budget, not blind full brute-force.
- Keep N-best decode candidates per hypothesis and select by calibrated score.

**Why:** high-version and distorted cases need robust, confidence-aware decode.

## Multi-QR Iteration (Stage E, `multi_qr_iteration`)

- Decode strongest QR first.
- Mask/discount its region and nearby proposals.
- Re-run Stage B-D on residual scene until no confident candidate remains.

**Why:** avoids mixing many QR hypotheses at once; scales better for dense images.

## Runtime Controller (to hit <=1s median)

- Replace one global hard deadline with staged budgets:
  - `proposal_ensemble` budget
  - `hypothesis_search` + `geometry_refinement` budget
  - `decode_engine` budget
  - `multi_qr_iteration` budget
  - Reserve budget for one last high-value retry lane
- Early accept for high confidence decodes.
- Early stop when marginal utility is low.
- Hard emergency cutoff for pathological scenes only.

## Metrics and Validation Design

Track per image:

- Proposal recall proxy (finders/quads found)
- Hypotheses generated vs kept
- Refinement convergence score
- Format extraction success rate
- RS success rate
- Final accepted decode confidence
- Time by stage

Key KPI gates:

1. `format-fail` must drop sharply before global rate can approach 90%.
2. `lots` and `rotations` must improve together; fixing only one is insufficient.
3. Runtime median must stay <=1s after each phase.

## Suggested Build Phases

## Phase 1 (2-3 weeks): Replace search + add geometry loop

- Implement finder graph + bounded beam hypotheses.
- Implement subpixel geometry refinement loop.
- Keep current decoder with minimal change initially.

**Expected gain:** large jump in `rotations`, moderate in `lots`/`high_version`.

## Phase 2 (2 weeks): Robust decode core

- Confidence-aware module sampling.
- Bounded decode hypothesis manager.
- Better format/version recovery from refined grids.

**Expected gain:** major `high_version`, plus broad lift from reduced `format-fail`.

## Phase 3 (1-2 weeks): Multi-QR iterative pipeline

- Residual scene decoding and region suppression.
- Category-aware but confidence-driven budgeting.

**Expected gain:** biggest jump in `lots`, with global impact due to 420 labels.

## Phase 4 (1 week): Runtime/quality optimization

- SIMD/parallel hotspots.
- Cache-friendly data flow.
- Remove redundant retries.

## Language / Tech Choices

- **Keep Rust** for production engine.
- Optionally prototype geometry/refinement logic quickly in Python first, then port.
- If needed, use a tiny ML proposal model later; keep decode core classical for control/debuggability.

## Risk Register

1. Overfitting to BoofCV categories.
2. Runtime blow-up in dense scenes.
3. Score calibration instability across categories.

Mitigations:

- Always run full-dataset CI artifact compare.
- Enforce per-stage time budgets.
- Keep ablation toggles for every major module.

## What I Would Do on Day 1

1. Build the stage-separated benchmark harness and failure dashboard.
2. Implement graph-based candidate search behind a feature flag.
3. Add minimal geometry refinement loop and measure `format-fail` change.
4. Only after that, tune thresholds.

---

This plan intentionally prioritizes architecture changes over incremental tuning because the current bottleneck is structural, not parameter-level.

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

## Proposed Architecture

## Stage A: Preprocess + Proposal Ensemble (fast, parallel)

- Build 3-4 binary views in parallel:
  - Otsu/global
  - Adaptive
  - Sauvola/contrast-aware
  - Optional glare-suppressed view
- Generate finder/quad proposals from all views.
- Union proposals with score normalization.

**Why:** maximize recall early with bounded extra cost.

## Stage B: Graph-Based Candidate Search (replace O(n^3) triplets)

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

## Stage C: Geometry Refinement Loop (critical)

- For each hypothesis:
  - Initialize homography
  - Refine corners/alignment with subpixel sampling
  - Fit against timing-line phase consistency
  - Re-estimate module pitch
- Run 2-3 lightweight refinement iterations.

**Why:** directly attacks current dominant `format-fail`.

## Stage D: Robust Decode (multi-hypothesis but bounded)

- For each refined grid:
  - Sample with subpixel interpolation
  - Produce soft module confidences
  - Decode with BCH/RS using confidence-aware retries
- Limit retries by confidence budget, not blind full brute-force.
- Keep N-best decode candidates per hypothesis and select by calibrated score.

**Why:** high-version and distorted cases need robust, confidence-aware decode.

## Stage E: Multi-QR Iterative Scene Decode (`lots` unlock)

- Decode strongest QR first.
- Mask/discount its region and nearby proposals.
- Re-run Stage B-D on residual scene until no confident candidate remains.

**Why:** avoids mixing many QR hypotheses at once; scales better for dense images.

## Runtime Controller (to hit <=1s median)

- Replace one global hard deadline with staged budgets:
  - Stage A budget
  - Stage B+C budget
  - Stage D budget
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


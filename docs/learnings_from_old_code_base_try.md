# Learnings from Old Codebase Try

**Date:** 2026-02-11  
**Dataset:** `benches/images/boofcv` (536 images, 1232 QR labels)  
**Latest fast benchmark reference:** GitHub Actions run `21911276395` (commit `12679ea6`, macOS)

## Snapshot

- Weighted global reading rate: **18.26%** (`225/1232`)
- Median runtime: **730.87 ms/image**
- Biggest category misses:
  - `lots`: **0.24%** (`1/420`)
  - `rotations`: **2.26%** (`3/133`)
  - `high_version`: **0.00%** (`0/37`)
  - `brightness`: **7.06%** (`6/85`)
  - `bright_spots`: **3.09%** (`3/97`)
- Dominant global failure signature: **`format-fail`** (`count=285`, `qr_weight=799`)

## What This Means

The pipeline often reaches finder/group/transform stages but fails at format extraction and robust decoding.  
This is not mainly a "can we find patterns?" issue anymore. It is mostly a **geometry + sampling + decode robustness** issue.

## Core Problems Seen in Current Approach

### 1. Early candidate truncation introduces search bias

- Group limits are applied at generation and ranking time.
- Correct triplets are often not in the first generated candidates, especially under rotation and dense multi-QR scenes.
- This causes false confidence in "we tried enough" when the search order itself is biased.

### 2. Router strategy is brittle for mixed scenes

- Rotation-heavy multi-QR images are easy to misroute.
- Routing based on early/top candidate signals can amplify upstream ranking errors.
- One wrong route decision can spend the image budget on the wrong decode lane.

### 3. O(n^3) grouping pressure leads to defensive caps

- Multi-finder scenes (`lots`, hard `rotations`) push combinatorics hard.
- Defensive caps prevent hangs but also kill recall.
- This is a structural tradeoff in the current grouping model.

### 4. Decode stage is too fragile to geometry error

- Large `format-fail` weight indicates transform/sampling accuracy is insufficient before decode.
- High-version and rotated cases require stronger subpixel alignment and timing/alignment consistency checks.
- Current decode retries consume time without enough transform correction feedback.

### 5. Category tuning loops create local optima

- Per-category thresholds and budgets can improve one bucket while regressing another.
- The system has many interacting knobs; manual tuning becomes unstable and expensive.
- Accuracy gains are no longer linear with effort.

### 6. Runtime control is too coarse at global level

- A single global budget protects CI but can cut off hard-but-decodable images.
- Budgeting should happen by stage and by confidence, not just by wall-clock deadline.

## Why We Hit a Wall

The architecture currently optimizes around constraints of the existing pipeline rather than changing the core search and geometry model.  
As a result, most effort goes into tuning guardrails around failure rather than removing the failure mode.

## Practical Takeaways

1. Keep Rust; language is not the bottleneck.
2. Stop broad threshold/budget tuning as the primary strategy.
3. Redesign candidate search (bounded graph search, not early-truncated triplet enumeration).
4. Redesign transform/sampling/format path to reduce `format-fail` first.
5. Treat `lots + rotations + high_version` as first-class design requirements, not fallback scenarios.

## Success Criteria for a New Direction

- `format-fail` should stop being the dominant failure cluster.
- `lots` should move from near-zero to high recall via true multi-QR iterative decode.
- `rotations` should not depend on uncapped brute-force candidate counts.
- `high_version` should improve through better geometry refinement and decode confidence modeling.


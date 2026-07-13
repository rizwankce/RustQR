# WP-012 controlled raster multi-QR scenes

This is the reproducible end-to-end density baseline for WP-012.  It is a
deliberately simple raster corpus: flat, axis-aligned Model 2 version-1,
EC-M symbols with five source pixels per module.  It does **not** represent
real multi-code photography.  Its purpose is to make density regressions and
the 50-code target measurable before adding camera artifacts.

## Corpus and evaluator

`scripts/generate_wp012_raster_scenes.py` creates 1, 2, 5, 10, 25, 50, and
100-symbol scenes under `tests/fixtures/wp012_raster_scenes/controlled_dense`.
The same-stem labels are strict BoofCV `SETS` quadrilaterals around the module
square (not the quiet zone).  `manifest.json` records the `python-qrcode`
backend version plus image and label hashes.

Run the release, native-resolution measurement with:

```bash
cargo build --release --features tools --bin qrtool
python3 scripts/evaluate_wp012_raster_scenes.py \
  --qrtool target/release/qrtool \
  --artifact artifacts/wp012_controlled_dense_by_density_qrmax0.json
```

The evaluator deliberately delegates each scene to `qrtool reading-rate`.
That is the normal public grayscale pipeline and its localization metric is
quadrilateral IoU >= 0.5 with maximum-cardinality bipartite matching.  Thus a
second result cannot inflate recall for one expected symbol; unmatched
overlapping predictions are accounted as duplicates and all other unmatched
predictions as false positives.

## 2026-07-13 bounded-region routing measurement

This measurement used `QR_MAX_DIM=0`, a 10,000 ms cooperative deadline, the
current release `qrtool`, and the checked-in `python-qrcode==8.2` manifest.
It follows the bounded-region routing change: a dense request visits one
region per retained candidate, capped at 128, instead of discarding all
regions beyond a fixed 32.

| Symbols | Matched | Recall | End-to-end ms | Symbols/s | FP | Duplicates |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1/1 | 100.00% | 2.84 | 351.59 | 0 | 0 |
| 2 | 2/2 | 100.00% | 5.24 | 381.43 | 0 | 0 |
| 5 | 5/5 | 100.00% | 306.61 | 16.31 | 0 | 0 |
| 10 | 10/10 | 100.00% | 815.54 | 12.26 | 0 | 0 |
| 25 | 23/25 | 92.00% | 2540.78 | 9.84 | 0 | 0 |
| 50 | 43/50 | 86.00% | 2139.27 | 23.37 | 0 | 0 |
| 100 | 61/100 | 61.00% | 409.61 | 244.13 | 0 | 0 |

Across all seven scenes the pipeline localized 145/193 symbols (75.13%) with
zero false positives and zero duplicates. Mean end-to-end runtime was 888.56
ms/image. The timings are one sample per deterministic scene, so they are a
measurement rather than a statistical latency claim.

The 50-symbol acceptance target (at least 90% recall under 500 ms) is **not
met**: this corpus gives 86% recall in 2139.27 ms. The flat-scene failure
rules out treating camera distortion as the main current blocker; candidate
sampling/ranking still needs work, and throughput remains well outside the
target.

## 2026-07-13 bounded strict-transform scheduling

Dense requests keep the full 25-point bottom-right transform sweep for the
first two recovery-eligible candidates. Every later retained candidate still
receives a strict ISO Model 2 decode, but uses a fixed center-plus-cardinal
five-point sweep; it does not consume the recovery transform frontier. This is
bounded candidate scheduling, not a relaxation of matrix validation or an
increase to the 128-candidate request cap.

A three-iteration in-process release probe of `dense_50` retained 48 decoded
symbols per call and reduced warm `detect` time from 1,779.29 ms to 414.72 ms.
Per-call allocations fell from 444,857 to 183,493 and requested bytes from
94,802,508 to 36,307,489. The public geometry evaluator recorded 43/50
(86.00%) in 455.85 ms with zero false positives, zero duplicates, and no
timeouts. Therefore the latency half of the 50-symbol target is met on this
controlled lane, but recall is still below 90%; WP-012 remains in progress.

## 2026-07-13 post-scheduling miss diagnosis

A fresh public-evaluator run was 43/50 (86.00%) in 485.14 ms, with zero false
positives, zero duplicates, and no timeout. The seven absent payloads were
`WP012-011`, `WP012-014`, `WP012-021`, `WP012-039`, `WP012-043`,
`WP012-046`, and `WP012-047`. Controlled finder-stage evidence remains 50/50,
so this is downstream of proposal recall.

The evaluator uses the bounded telemetry entry point. Its white background
selects the existing brightness shortcut, whose first successful dense decode
returns 43 results. The ordinary direct API's fast Otsu path returns 48 valid,
distinct Model 2 payloads on the same raster, but this local probe took about
690 ms. It cannot be promoted into the evaluator as-is because that violates
the controlled 500 ms budget.

This evidence does not justify relaxing matrix validation, RS recovery, or the
128-candidate request cap. The next bounded experiment is per-candidate
brightness-route instrumentation followed by a time-capped strict
Otsu-derived geometry/sampling variant. It must reach >=45/50 with zero false
positives, zero duplicates, and <500 ms in the public evaluator before any
default-route change.

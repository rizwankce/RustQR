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

## 2026-07-13 release baseline

This baseline used `QR_MAX_DIM=0`, a 10,000 ms cooperative deadline, release
`qrtool` at `096d31e`, and the checked-in `python-qrcode==11.1.0` manifest.

| Symbols | Matched | Recall | End-to-end ms | Symbols/s | FP | Duplicates |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1/1 | 100.00% | 3.50 | 285.65 | 0 | 0 |
| 2 | 2/2 | 100.00% | 4.26 | 469.25 | 0 | 0 |
| 5 | 3/5 | 60.00% | 92.41 | 54.10 | 0 | 0 |
| 10 | 3/10 | 30.00% | 105.60 | 94.69 | 0 | 0 |
| 25 | 2/25 | 8.00% | 1508.75 | 16.57 | 0 | 0 |
| 50 | 2/50 | 4.00% | 3607.59 | 13.86 | 0 | 0 |
| 100 | 1/100 | 1.00% | 511.24 | 195.60 | 0 | 0 |

Across all seven scenes the pipeline localized 14/193 symbols (7.25%) with
zero false positives and zero duplicates.  Mean end-to-end runtime was
782.61 ms/image.  The timings are one sample per deterministic scene, so they
are a baseline rather than a statistical latency claim.

The 50-symbol acceptance target (at least 90% recall under 500 ms) is **not
met**: this corpus gives 4% recall in 3607.59 ms.  The flat-scene failure rules
out treating camera distortion as the main current blocker; ROI-first raster
processing, higher dense-scene finder recall, and region-aware decode routing
remain required.

# WP-005 targeted normalized branch comparison

## Scope

This is a local, detached-worktree comparison of historical `main@5b9b41e`
and `scratch_from_scratch_rebuild@295b97c`.  It uses the reviewed throwaway
exporters and the main-only measured-geometry projection patch.  The active
decoder was not changed.

The stream contains the deterministic first 25 images in each requested
category, except `lots`, which contains all 7 available images.  That is 157
images and 727 BoofCV labels total.  Every category was exported in
non-overlapping five-image `--offset` shards, then merged only after exact
ID, dimensions, dataset-fingerprint, and preprocessing-fingerprint checks.

- Dataset fingerprint: `fnv1a64:812bbf57f2ce9c36`
- Preprocessing: `rgb8;triangle-resize;max-dim=1024`
- Ordered image-ID SHA-256:
  `3ada84f4809550c03b9aeae4d13f38c50e2d1b0d8bdd23734e88a766eff313bc`
- Main prediction SHA-256:
  `f126f59deba999a4694dcf00c73992824dc93e36e4cadc4903148622270d62ff`
- Rebuild prediction SHA-256:
  `fc7469523d04e969e533fb78b23ffbf94da341e6bd378d66ec106f055bb7bbc7`
- Truth SHA-256:
  `f533d297847fe697f792039c568cfedd7991230733bfcc35c247d0de435e1cb3`

## Results

| Category | Main hits / labels | Rebuild hits / labels | Main rate | Rebuild rate | Main median e2e | Rebuild median e2e |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| bright_spots | 0 / 76 | 0 / 76 | 0.00% | 0.00% | 2328.490 ms | 297.429 ms |
| brightness | 5 / 76 | 6 / 76 | 6.58% | 7.89% | 2289.120 ms | 299.742 ms |
| high_version | 0 / 25 | 1 / 25 | 0.00% | 4.00% | 2243.218 ms | 207.946 ms |
| lots | 1 / 420 | 0 / 420 | 0.24% | 0.00% | 2312.696 ms | 305.374 ms |
| nominal | 18 / 29 | 18 / 29 | 62.07% | 62.07% | 1690.258 ms | 130.058 ms |
| perspective | 10 / 25 | 20 / 25 | 40.00% | 80.00% | 2032.300 ms | 132.778 ms |
| rotations | 2 / 76 | 29 / 76 | 2.63% | 38.16% | 2220.334 ms | 192.867 ms |
| **Total** | **36 / 727** | **74 / 727** | **4.95%** | **10.18%** | **2167.191 ms** | **192.968 ms** |

Main had one false positive (in `perspective`); neither stream had duplicate
predictions or timeouts. The rebuild gains 38 labels overall, with most of the
gain in rotations (+27) and perspective (+10); it loses the single `lots`
label.  The measured timing belongs to different historical binaries on this
machine and is diagnostic, not a cross-run performance claim.

## Reproduction and limits

The retained raw streams and normalized outputs are:

- `wp005_main_targeted_1024_limit25_prediction_stream.json`
- `wp005_rebuild_targeted_1024_limit25_prediction_stream.json`
- `wp005_truth_targeted_1024_limit25.json`
- `wp005_main_targeted_1024_limit25_v2.json`
- `wp005_rebuild_targeted_1024_limit25_v2.json`

They were produced with `--root benches/images/boofcv`, the repeated seven
`--category` names, `--limit 5`, offsets `0,5,10,15,20` (only `0,5` for
`lots`), `--timeout-ms 0`, then a metadata-normalized merge with
`limit_per_category=25`. `make_wp005_truth_manifest.py` produced the shared
truth manifest; `normalize_wp005_prediction_stream.py` scored each branch.
`compare_reading_rate_artifacts.py` accepted the complete seven-category
coverage with exploratory 100pp per-category drop bounds.

This closes the local normalized-comparison evidence gap, but it does **not**
meet the Actions acceptance gate: the same Fast Benchmark configuration has
not yet run remotely for both commits, and this does not authorize merging or
transplanting rebuild code into the active branch.

# WP-010 finder and grouping stage evidence

This document records the first reproducible, stage-separated measurement for
WP-010. It does not measure transform construction, sampling, Reed-Solomon, or
payload correctness.

## Evaluator

Run the release evaluator with the same resize policy used by the recorded
artifacts: QR_MAX_DIM=800 cargo run --release --features tools --bin qrtool
-- proposal-eval --smoke --artifact-json
artifacts/wp010_proposal_grouping_eval_boofcv_smoke_qrmax800.json.

qrtool proposal-eval parses the existing BoofCV quadrilateral labels, scales
them to the processed raster, then measures:

- **finder recall**: a labeled symbol contains at least three retained
  FinderDetector::detect_proposals centres;
- **grouping recall**: a legacy group_finder_patterns triplet has all three
  centres inside one labeled symbol;
- **spurious proposals/groups**: a retained centre or triplet is not fully
  contained by any labeled symbol;
- separate scan/rank/NMS and grouping wall-clock intervals.

Containment deliberately avoids deriving finder centres from an unknown QR
version or orientation. It is a stage diagnostic, not an IoU localization
metric and not a final-decode accuracy claim.

## Recorded release runs

All runs used QR_MAX_DIM=800 and the checked-in BoofCV labels.

| Slice | Images | Symbols | Finder recall | Group recall | Proposal mean/p50/p95 ms | Grouping mean/p50/p95 ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| smoke | 12 | 17 | 12/17 (70.59%) | 7/17 (41.18%) | 3.809/4.143/5.797 | 0.006/0.006/0.010 |
| nominal | 65 | 78 | 71/78 (91.03%) | 59/78 (75.64%) | 3.734/3.728/5.784 | 0.005/0.005/0.012 |
| lots | 7 | 420 | 80/420 (19.05%) | 3/420 (0.71%) | 5.348/5.201/6.216 | 0.017/0.025/0.035 |
| lots, ROI/spatial follow-up | 7 | 420 | 80/420 (19.05%) | 52/420 (12.38%) | 5.115/4.994/5.513 | 0.551/0.547/1.322 |

The matching JSON artifacts are:

- artifacts/wp010_proposal_grouping_eval_boofcv_smoke_qrmax800.json
- artifacts/wp010_proposal_grouping_eval_nominal_qrmax800.json
- artifacts/wp010_proposal_grouping_eval_lots_qrmax800.json
- artifacts/wp010_proposal_grouping_eval_lots_roi_spatial_qrmax800.json

## Interpretation and remaining work

The initial dense lots result established that the proposal plus legacy
grouping path did not meet WP-010's multi-symbol requirement.  The ROI/spatial
follow-up retains exact three-proposal local components before bounded
neighbourhood expansion and raises grouping recall from 3/420 to 52/420 at the
same proposal recall.  It is still evidence against treating the proposal
boundary as complete: the dominant dense-scene loss is now finder recall
(80/420), and 12.38% grouping recall is far below the acceptance target.

The remaining WP-010 implementation work is unchanged:

1. replace allocation-heavy whole-raster row/column scan loops with
   allocation-light scanline state machines;
2. introduce raster ROI processing before proposal expansion in dense scenes,
   with bounded per-region proposal/group budgets;
3. validate and tune the new region-aware grouping path against the full
   corpus and real decoded dense scenes;
4. rerun this evaluator across the full labeled corpus and add the project's
   agreed category-regression thresholds before claiming the acceptance gates.

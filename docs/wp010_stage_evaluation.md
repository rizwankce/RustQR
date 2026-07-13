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

The matching JSON artifacts are:

- artifacts/wp010_proposal_grouping_eval_boofcv_smoke_qrmax800.json
- artifacts/wp010_proposal_grouping_eval_nominal_qrmax800.json
- artifacts/wp010_proposal_grouping_eval_lots_qrmax800.json

## Interpretation and remaining work

The dense lots result establishes that the current proposal plus legacy
grouping path does not meet WP-010's multi-symbol requirement. In particular,
the evaluator observes only 3 grouped symbols out of 420 labeled symbols;
that is evidence against treating the current proposal boundary as complete.

The remaining WP-010 implementation work is unchanged:

1. replace allocation-heavy whole-raster row/column scan loops with
   allocation-light scanline state machines;
2. introduce ROI-first region processing before proposal expansion in dense
   scenes, with bounded per-region proposal/group budgets;
3. replace the legacy global grouping path with region-aware grouping that
   preserves independent symbols;
4. rerun this evaluator across the full labeled corpus and add the project's
   agreed category-regression thresholds before claiming the acceptance gates.

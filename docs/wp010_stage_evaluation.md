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

Version 3 of the artifact additionally records the number of contained finder
centres per annotation (zero, one, two, and three-or-more) and the number of
finder-stage-eligible annotations with no contained group. It therefore
identifies whether a miss occurred before grouping or during grouping. It also
separates the retained proposals and
groups that are contained by a label from ones that are spurious.  It reports
contained duplicate groups independently: only the first contained group for a
label counts toward grouping recall, so the duplicates represent extra
downstream work rather than extra recall.  These are containment diagnostics,
not final QR precision measurements.

Version 4 also records candidate-backed ROI recovery windows, rows, columns,
and raw observations. Each image can select no more than twelve 192px windows;
this keeps the exact-edge supplemental scan bounded and auditable.

Version 6 additionally records the dense contour-family supplement. It runs
only once after a dense scan, requires independent horizontal/vertical/pitch
finder evidence, preserves the primary scanline proposal order, and appends at
most 128 non-overlapping secondary proposals.

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
| lots, current containment diagnostic (v2) | 7 | 420 | 80/420 (19.05%) | 76/420 (18.10%) | 5.277/5.174/5.579 | 6.904/7.483/16.651 |
| lots, bounded ROI proposal recovery (v4) | 7 | 420 | 81/420 (19.29%) | 77/420 (18.33%) | 10.462/11.703/20.390 | 7.293/7.709/19.622 |
| lots, bounded contour supplement (v6) | 7 | 420 | 102/420 (24.29%) | 87/420 (20.71%) | 13.292/19.349/21.577 | 6.736/9.434/14.554 |

The fresh v3 `lots` run has the same 80 finder-stage-eligible and 76 grouped
symbols, but makes the loss boundary explicit: **263/420 annotations have zero
contained proposal centres, 36 have one, 41 have two, and 80 have three or
more**. Only **4/80** finder-stage-eligible symbols fail to produce a contained
group. It also retains 376 contained versus 41 spurious proposals and emits
83 contained groups (7 duplicates) versus 440 spurious groups. Thus proposal
recall, not the bounded grouping transition, accounts for 340 of 344 dense
stage misses. The v3 release run measured proposal mean/p50/p95 latency of
5.339/5.261/5.814 ms and grouping 6.589/7.573/15.320 ms.

The matching JSON artifacts are:

- artifacts/wp010_proposal_grouping_eval_boofcv_smoke_qrmax800.json
- artifacts/wp010_proposal_grouping_eval_nominal_qrmax800.json
- artifacts/wp010_proposal_grouping_eval_lots_qrmax800.json
- artifacts/wp010_proposal_grouping_eval_lots_roi_spatial_qrmax800.json
- artifacts/wp010_proposal_grouping_eval_lots_containment_v3_qrmax800.json
- artifacts/wp010_proposal_grouping_eval_lots_roi_recovery_v4_qrmax800.json
- artifacts/wp010_proposal_grouping_eval_lots_contour_recovery_v6_qrmax800.json

## Interpretation and remaining work

The initial dense lots result established that the proposal plus legacy
grouping path did not meet WP-010's multi-symbol requirement.  The ROI/spatial
follow-up retains exact three-proposal local components before bounded
neighbourhood expansion and raises grouping recall from 3/420 to 52/420 at the
same proposal recall.  The current v2 diagnostic records 76/420 grouping
recall after the bounded dense-routing changes, but also records 83 contained
groups, 7 contained duplicates, and 440 spurious groups.  This confirms that
the proposal boundary is not complete: finder recall remains 80/420, and the
grouping output still requires substantially better selectivity before it can
meet the acceptance target. The v3 artifact's multiplicity histogram means a
follow-up can target raster proposal recovery when most annotations lack three
retained centres, rather than incorrectly tuning grouping.

The bounded ROI recovery pass is a measured, detector-only experiment. It
selects candidate-populated cells after the normal scan and rescans them with
an exact rather than 4-pixel edge gate. On `lots`, it adds one finder-stage
and one grouping-stage hit (80→81 and 76→77), raises contained proposals
376→377, and leaves spurious proposals at 41. Across seven images it used 48
windows, 9,874 row scans, 10,284 column scans, and 790 supplemental raw
observations; the per-image cap is independently unit-tested. It retains all
193/193 controlled dense symbols at the finder stage. The small gain and
higher proposal latency mean this is evidence for a more selective future ROI
proposal strategy, not acceptance completion.

The v6 contour supplement is the first material proposal-stage improvement
after the ROI experiment. Its 189 independently cross-checked raw contour
observations yielded 112 appended, non-overlapping proposals across the seven
`lots` images. Finder recall rose 81→102 and grouping recall 77→87, while
spurious proposals rose only 41→44. It retains all 193 controlled dense
symbols at the finder stage. The detector still has 215 symbols with zero
contained proposals and is far from the full-corpus acceptance gate; this is
bounded evidence, not a production recall claim.

The remaining WP-010 implementation work is unchanged:

1. replace allocation-heavy whole-raster row/column scan loops with
   allocation-light scanline state machines;
2. introduce raster ROI processing before proposal expansion in dense scenes,
   with bounded per-region proposal/group budgets;
3. validate and tune the new region-aware grouping path against the full
   corpus and real decoded dense scenes;
4. rerun this evaluator across the full labeled corpus and add the project's
   agreed category-regression thresholds before claiming the acceptance gates.

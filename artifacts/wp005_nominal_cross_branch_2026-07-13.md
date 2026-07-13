# WP-005 normalized nominal cross-branch evidence

This is a bounded local evidence slice, not the required seven-category or
Fast Benchmark comparison.

- Pinned refs: `main@5b9b41e` and
  `scratch_from_scratch_rebuild@295b97c`.
- Pixels: the same 25 sorted `nominal` images from
  `benches/images/boofcv` at 1024 px maximum, selected through the identical
  explicit category allow-list.
- Dataset fingerprint: `fnv1a64:812bbf57f2ce9c36`.
- Labels: 29 total, shared label fingerprint
  `af223ad3fe503d6d3ceaed8cae8a17b9b2bcc213bdc07f38c2c92ebdf413078b`.
- Evaluator: `mode=localization;quad-iou=0.5;matching=max-cardinality`.
- Sharding: five non-overlapping five-image exporter shards per ref were
  merged only after exact image-ID and metadata-fingerprint checks. The
  retained prediction streams record `limit_per_category: 25`.

| Ref | Hits / labels | Rate | FP / duplicates / timeouts | Median end-to-end |
| --- | ---: | ---: | ---: | ---: |
| `main@5b9b41e` | 18 / 29 | 62.07% | 0 / 0 / 0 | 1690.258 ms |
| `scratch_from_scratch_rebuild@295b97c` | 18 / 29 | 62.07% | 0 / 0 / 0 | 130.058 ms |

The standalone comparator reports a zero nominal accuracy delta and a
92.31% lower rebuild median runtime on this machine. It correctly exits
nonzero because the required `rotations`, `high_version`, and `lots` category
gates are absent. Therefore this artifact makes no adoption, regression, or
whole-workload performance claim.

Retained raw and normalized artifacts:

- `wp005_main_nominal_1024_limit25_prediction_stream.json`
- `wp005_rebuild_nominal_1024_limit25_prediction_stream.json`
- `wp005_truth_nominal_1024_limit25.json`
- `wp005_main_nominal_1024_limit25_v2.json`
- `wp005_rebuild_nominal_1024_limit25_v2.json`

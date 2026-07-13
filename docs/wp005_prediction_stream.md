# WP-005 throwaway prediction-stream contract

This contract closes the *adapter* gap identified in ADR-0001; it does not
create a branch comparison by itself. It is intentionally external to all
decoder branches. `main`, `scratch_from_scratch_rebuild`, and the current
branch must each export the same stream from clean, throwaway adapter refs.

## Prediction export

An adapter writes `rustqr.wp005.prediction-stream.v1` with this required shape:

```json
{
  "schema_version": "rustqr.wp005.prediction-stream.v1",
  "metadata": {
    "commit_sha": "adapter-source-commit",
    "dataset_fingerprint": "sha-or-equivalent",
    "preprocessing_fingerprint": "rgb8;triangle-resize;max-dim=1024",
    "limit_per_category": 25
  },
  "images": [{
    "image_id": "nominal/image001.jpg",
    "category": "nominal",
    "original_width": 1920,
    "original_height": 1080,
    "working_width": 1024,
    "working_height": 576,
    "predicted_quadrilaterals": [[[0, 0], [1, 0], [1, 1], [0, 1]]],
    "payloads": ["optional decoded UTF-8 payload"],
    "core_elapsed_ms": 12.3,
    "end_to_end_elapsed_ms": 18.7,
    "timed_out": false
  }]
}
```

`predicted_quadrilaterals` are mandatory even when payloads are unavailable,
must use original-image coordinates, and must include every prediction. A
timed-out row still records observed elapsed time but contributes no prediction
to scoring. Adapters must not write a nonempty/image-success boolean in place
of geometry. The metadata fingerprints and dimensions prevent mixed pixels,
resize policies, or coordinate spaces from being silently compared.

A separate shared `rustqr.wp005.localization-truth.v1` manifest supplies the
same image identity, category, dimensions, original-coordinate BoofCV label
quadrilaterals, dataset fingerprint, label fingerprint, and preprocessing
fingerprint. It is produced once from the selected fixed input list, not by a
candidate branch.

## Normalization

Run the shared scorer only after one prediction export and its shared truth
manifest exist:

```sh
python3 scripts/normalize_wp005_prediction_stream.py \
  --predictions wp005_main_predictions.json \
  --truth wp005_truth_1024_limit25.json \
  --output wp005_main_targeted_v2.json
```

The normalizer requires exact image coverage and dimensions, checks matching
pixel/preprocessing fingerprints, applies
`mode=localization;quad-iou=0.5;matching=max-cardinality`, and emits the v2
summary/category fields consumed by
`scripts/compare_reading_rate_artifacts.py`. It counts false positives,
false negatives, duplicate predictions, and timeouts with the current v2
semantics. It never runs a decoder and must not be merged into a candidate
decoder branch.

The included parser tests use synthetic geometry only. They establish the
protocol (maximum-cardinality duplicate handling, timeout discard, and
dimension rejection), not real cross-branch accuracy or latency.

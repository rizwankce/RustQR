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

## Current-branch exporter

The current branch provides this contract without changing decoder behavior:

```sh
QR_MAX_DIM=1024 cargo run --release --features tools --bin qrtool -- \
  prediction-export --category nominal --limit 1 --timeout-ms 0 \
  --output artifacts/wp005_current_nominal_limit1_prediction_stream.json

python3 scripts/make_wp005_truth_manifest.py \
  --predictions artifacts/wp005_current_nominal_limit1_prediction_stream.json \
  --dataset-root benches/images/boofcv \
  --output artifacts/wp005_truth_nominal_limit1_1024.json

python3 scripts/normalize_wp005_prediction_stream.py \
  --predictions artifacts/wp005_current_nominal_limit1_prediction_stream.json \
  --truth artifacts/wp005_truth_nominal_limit1_1024.json \
  --output artifacts/wp005_current_nominal_limit1_v2.json
```

`prediction-export` reuses `load_rgb_with_geometry`, the normal detector, and
the existing cooperative timeout path. It records all result positions after
rescaling working coordinates back to original-image coordinates. The truth
helper reads only same-stem BoofCV `SETS` label files selected by the export;
it writes a separate manifest and label fingerprint, never candidate-derived
labels.

The checked-in local artifact pair above covers only `nominal/image001.jpg`.
Its normalized result is 0/2 with no false positives, because the current
detector exported no predictions for that image. That is a contract smoke,
not an accuracy result or a comparison with `main` or the rebuild.

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

## Portability audit and throwaway-adapter setup (2026-07-13)

The current `prediction-export` command is **not** a literal source port to
either comparison ref. This is an interface audit only; no target ref was
checked out, changed, built, or compared.

| Ref | Pinned commit | Reusable public API | Required throwaway glue |
|---|---|---|---|
| `main` | `5b9b41e` | `rust_qr::detect` returns `QRCode { content, position }`; `image` and Triangle resize already exist under `tools` | Preserve original dimensions before calling `tools::load_rgb`, then scale `position` back to source coordinates. Add the stream writer as a temporary `src/bin/wp005_prediction_export.rs`; do not modify `qrtool` or decoder code. |
| `scratch_from_scratch_rebuild` | `295b97c` | `rust_qr::detect` returns `QrCode { payload, corners }`; the rebuild's tool loader supports explicit Triangle resize | Add the same temporary bin, call `tools::load_rgb_image(path, Some(1024))`, map `corners`, and scale them to source coordinates. Its hand-written `qrtool` has no Clap subcommand mechanism, so do not attempt to transplant the current command enum. |

The shared adapter body must itself own deterministic recursive image
enumeration, original-image RGB dimensions, JSON escaping, FNV dataset
fingerprinting, output-path creation, and JSON serialization. It accepts
explicit `--root`, one or more `--category` values, `--limit`, `--timeout-ms`,
`--output`, and `--commit-sha`. Requiring categories prevents an accidental
full-tree export from silently changing the comparison scope.

`--offset N` is optional and skips the first `N` sorted images within each
selected category before applying `--limit`. It exists solely to shard a fixed
selection where a process deadline makes a single exporter invocation
impractical. Merge only non-overlapping shards with the same root, category
allow-list, limit, commit, and metadata fingerprints; the merged rows must
cover the selected IDs exactly once before truth generation.
arguments. For these pinned target refs, `--timeout-ms` must be `0`: neither
target exposes the current request-timeout API, and fabricating timeout
semantics would invalidate the stream. The preprocessing fingerprint is
exactly `rgb8;triangle-resize;max-dim=1024`; its loader and every coordinate
conversion must be checked against that value before a stream is retained.

Run this later from a disposable directory only, after copying the reviewed
branch-specific adapter source into each throwaway worktree. The commands are
intentionally not a comparison and must not be run in the active worktree:

```sh
git worktree add --detach /tmp/rustqr-wp005-main main
git worktree add --detach /tmp/rustqr-wp005-rebuild scratch_from_scratch_rebuild

# In each throwaway worktree, copy the matching reviewed adapter to
# src/bin/wp005_prediction_export.rs, then build and export the same slice.
cd /tmp/rustqr-wp005-main
cargo run --release --features tools --bin wp005_prediction_export -- \
  --root benches/images/boofcv \
  --category nominal --category rotations --category perspective \
  --category high_version --category lots --category brightness \
  --category bright_spots --limit 25 --timeout-ms 0 \
  --commit-sha 5b9b41e --output /tmp/wp005_main_predictions.json

cd /tmp/rustqr-wp005-rebuild
cargo run --release --features tools --bin wp005_prediction_export -- \
  --root benches/images/boofcv \
  --category nominal --category rotations --category perspective \
  --category high_version --category lots --category brightness \
  --category bright_spots --limit 25 --timeout-ms 0 \
  --commit-sha 295b97c --output /tmp/wp005_rebuild_predictions.json
```

Before normalization, verify that the two exports enumerate exactly the same
`image_id` list and share dataset/preprocessing fingerprints. Produce a single
truth manifest from that fixed list, normalize each stream independently, and
only then use the v2 comparator. A successful adapter build/export proves
serialization compatibility only; it is neither an accuracy comparison nor a
latency claim.

## Pinned-adapter smoke (2026-07-13)

Reviewed source files now live in `scripts/wp005_adapters/` and were copied
unchanged into detached worktrees at the two pinned commits. Both release bins
built with their own historical dependency sets and exported the same
`nominal/image001.jpg` 1024px-or-smaller slice. Both streams have the same
image ID, `fnv1a64:d684f3ec5eccd229` dataset fingerprint, and the required
preprocessing fingerprint. A single truth manifest made from that fixed slice
successfully normalized each stream through the shared v2 scorer. The retained
evidence is `artifacts/wp005_pinned_adapter_smoke_2026-07-13.md`.

`main@5b9b41e` can return a decoded payload with its public `position` left at
four zero points. That has no localizable quadrilateral, and the normalizer
correctly rejects it as zero-area geometry. The main adapter therefore drops
that payload-and-geometry pair together; it never emits a fabricated box or a
payload-only prediction. This makes the smoke stream valid but means a full
comparison needs a main geometry-producing path or must explicitly record the
resulting localization loss. The smoke is adapter compatibility evidence only;
both normalized results are 0% localization rate on this one two-label image.

## Main geometry projection probe (2026-07-13)

The historical main pipeline retains measured finder-triplet geometry for the
candidate that decoded a QR, but does not copy it into the public
`QRCode.position`. A reviewed throwaway-only patch now lives at
`scripts/wp005_adapters/main_5b9b41e_geometry_projection.patch`. It projects
the outer QR boundary through that candidate's finder centers and decoded
dimension; it does not infer a box from payload presence or labels. Applied
only in the detached `main@5b9b41e` worktree, it produced nonzero geometry
that normalized as 17/17 hits on the monitor directory, with zero false
positives, false negatives, duplicates, or timeouts. Full evidence and the
exact patch application command are in
`artifacts/wp005_main_geometry_monitor_2026-07-13.md`.

The monitor command deliberately exposed an adapter selection caveat: passing
`--root benches/images/boofcv/monitor --limit 1` treats each filename as a
separate category and therefore exports all 17 images. This is useful probe
evidence, not a one-image result. The seven-category comparison must pass the
explicit repeated `--category` allow-list above from
`--root benches/images/boofcv --limit 25`. The adapters reject invocations
without that allow-list, enumerate deterministic sorted IDs, and take the same
prefix per category. Verify matching image-ID lists before shared-truth
normalization.

## Bounded nominal comparison (2026-07-13)

Both pinned historical adapters now require the same repeated `--category`
allow-list. On an environment with a short per-process command window, the
adapter's optional `--offset` produced five non-overlapping five-image shards
for the 25-image `nominal` selection. The merged streams passed exact ID,
dimension, and metadata-fingerprint checks before one shared truth manifest
and v2 normalization.

`main@5b9b41e` and `scratch_from_scratch_rebuild@295b97c` each hit 18 of 29
nominal labels (62.07%), with no false positives, duplicate predictions, or
timeouts. Their local median end-to-end times were respectively 1690.258 ms
and 130.058 ms. This is a bounded diagnostic only: the standard comparator
correctly fails the partial artifacts because `rotations`, `high_version`, and
`lots` are missing. The exact raw streams, normalized artifacts, and caveats
are retained in `artifacts/wp005_nominal_cross_branch_2026-07-13.md`.

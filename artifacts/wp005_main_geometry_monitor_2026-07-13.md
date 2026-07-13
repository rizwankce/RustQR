# WP-005 main geometry projection evidence

Date: 2026-07-13

This is a throwaway-adapter feasibility result for historical
`main@5b9b41e`. It is not a comparison with the rebuild or current branch,
and it does not change the active decoder.

## Geometry source

`main` already decodes through `pipeline::decode_candidate`, where each
successful decode has measured finder-pattern centers (`TL`, `TR`, and `BL`),
module size, and decoded QR dimension. The historical `QRCode` constructor
leaves public `position` at four zero points, even when that candidate
produced a payload. The retained patch
`scripts/wp005_adapters/main_5b9b41e_geometry_projection.patch` projects the
outer module boundary through those same measured finder points:

- finder centers map from module coordinates `(3.5, 3.5)`,
  `(dimension - 3.5, 3.5)`, and `(3.5, dimension - 3.5)`;
- the fourth destination point is the candidate's geometric parallelogram
  corner, `TR + BL - TL`;
- output points are the projected module boundary in clockwise BoofCV order:
  `TL`, `TR`, `BR`, `BL`.

It does not derive a box from payload presence, labels, or an image-wide
heuristic. The patch is only for a detached historical worktree and must not
be applied to the active branch as an adapter shortcut.

## Reproduction and monitor result

In `/private/tmp/rustqr-wp005-main`, after copying the reviewed main exporter:

```sh
git apply /path/to/scripts/wp005_adapters/main_5b9b41e_geometry_projection.patch
cargo fmt -- src/pipeline.rs src/bin/wp005_prediction_export.rs
cargo run --release --features tools --bin wp005_prediction_export -- \
  --root benches/images/boofcv/monitor --limit 1 --timeout-ms 0 \
  --commit-sha 5b9b41e --output /tmp/wp005-main-monitor-geometry.json
```

The first `monitor/image001.jpg` prediction was a nonzero original-coordinate
quadrilateral near its BoofCV label and normalized as a localization hit. The
actual command selected all 17 monitor images, because the adapter treats the
first component of each path relative to `--root` as its category. With
`--root benches/images/boofcv/monitor`, each filename became its own category,
so `--limit 1` did not limit the directory to one image. The resulting
17-image monitor-only artifact normalized to 17/17 hits, zero false positives,
false negatives, duplicates, or timeouts. Core median was 558.005 ms and p95
was 2429.665 ms. These timings are diagnostic only and are not a matched
cross-branch latency claim.

## Selection guard

For the intended seven-category, 25-per-category comparison, the exporter
must use `--root benches/images/boofcv`, never a single category directory.
At that root its per-category counter is correct, but the reviewed historical
adapter has no category allow-list and would also export the other dataset
directories. Therefore the exact seven-category selection is still blocked on
adding the same explicit allow-list mechanism to each throwaway adapter (or on
using a separately materialized fixed input tree). Before scoring, verify the
exact selected image-ID lists match across all three adapter exports.

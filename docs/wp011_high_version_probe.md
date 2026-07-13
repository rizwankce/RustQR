# WP-011 high-version local probe

## Scope

This is a deliberately small local diagnostic, not a reading-rate baseline or
an acceptance claim. It used the checked-in BoofCV images at `QR_MAX_DIM=800`
and the CLI's 2.5 second cooperative deadline.

```sh
QR_MAX_DIM=800 cargo run --features tools --bin qrtool --release -- \
  reading-rate --category high_version --limit 1 --non-interactive \
  --timeout-ms 2500 --artifact-json /tmp/wp011_high_version_1_2500.json
```

## Result

`high_version/image000.jpg` produced no localization match. The saved artifact
reports one timeout, 29 decode attempts, 2,925 high-version subpixel samples,
48 refinement attempts, and no successful refinement. Core runtime was
11,630.31 ms; end-to-end runtime was 11,680.04 ms. The deadline correctly
discarded its late result for scoring, but it did not preempt the work.

This explains why a multi-image `high_version` probe is unsuitable as an
interactive local gate at the current implementation: even a one-image run is
about 4.7 times the requested 2.5 second budget. A three-image run was started
with the same command and limit changed to 3, but did not finish within the
interactive runner's bounded observation window, so it is intentionally not
reported as a category result. The prior recorded `0/3` observation in
`TODO.md` remains evidence only, not a current before/after measurement.

## Control-flow finding

The request deadline is passed to decoder recovery through
`DecodeRequestContext`, whose recovery loops poll it. It is not, however,
polled at the start of each primary binarization-policy iteration in
`detect_with_telemetry_budget`, nor can the current `binarize_with_policy` and
`FinderDetector::detect[_with_pyramid]` APIs stop part-way through their image
scans. After a late decode returns empty, the outer policy loop can therefore
continue through additional adaptive binarization and finder scans before the
later contour-fallback loop reaches its deadline check.

This is a pipeline-wide cooperative-cancellation gap, not a narrowly scoped
geometry refinement. It is intentionally left unchanged by this WP-011 probe
to avoid silently changing WP-010/WP-014 pipeline ownership. It also means
the existing timeout telemetry must be read as "late result discarded", not as
a hard per-image latency cap.

## Next bounded implementation slice

Before claiming candidate-level deadline behavior, pass the same request
deadline into the outer policy scheduler and stop before starting each new
binarization/finder/contour/ROI-normalization operation. A later, separately
scoped pipeline change can add periodic cancellation polling to the finder and
binarization loops. Re-run the one-image probe first; only then collect
`high_version --limit 3` and `--limit 5` artifacts under an explicit external
wall-clock harness if hard timing evidence is required.

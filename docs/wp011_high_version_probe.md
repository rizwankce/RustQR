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
`DecodeRequestContext`, whose recovery loops poll it. The scheduler now also
checks the request gate before each primary binarization pass, after each
uninterruptible binarization before starting its finder scan, before contour
fallback scans, and before ROI normalization work. Cancellation uses the same
gate. A deterministic zero-deadline unit test verifies that no binarization or
finder pass is scheduled for an already-expired request.

The current `binarize_with_policy` and `FinderDetector::detect[_with_pyramid]`
APIs still cannot stop part-way through an image scan. Thus this closes the
extra-pass scheduling gap but does not turn the deadline into a hard per-image
latency cap; timeout telemetry still means that an in-flight operation may
have returned late and its result was discarded.

## Scheduler recheck

After the scheduler change, the same one-image command completed with the
same 0/1 result and one timeout. The saved local artifact at
`/tmp/wp011_high_version_scheduler_1_2500.json` recorded 3,406.17 ms core and
3,455.38 ms end-to-end, with 11 decode attempts, 420 high-version subpixel
samples, six refinement attempts, and no refinement success. This single
recheck demonstrates that no later primary-policy fallback was scheduled (the
fallback-transition counters were both zero); it is not an attributable
before/after performance comparison because other in-progress pipeline work
shares the worktree, nor is it a category acceptance result.

## Next bounded implementation slice

Before claiming a hard per-operation deadline, add periodic cancellation
polling to the finder and binarization loops. Re-run the one-image probe first;
only then collect
`high_version --limit 3` and `--limit 5` artifacts under an explicit external
wall-clock harness if hard timing evidence is required.

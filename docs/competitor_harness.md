# OSS competitor harness

WP-013 compares competitors only through
`scripts/run_competitor_harness.py`. It has a deliberately narrower contract
than the RustQR evaluator: it establishes reproducible inputs, timeout and
timing boundaries, raw observations, and count-based annotation coverage. It
does not call a returned payload a correct payload without a payload-labelled
case manifest, and it does not fabricate localization metrics from CLIs that
do not expose quadrilaterals.

## Contract

The harness decodes each source image once with Pillow, converts it to BT.601
8-bit luma, applies the pinned Triangle resize to `--max-dim` (default 1024),
and writes a temporary binary PGM. Every adapter receives the same PGM bytes;
the report includes their SHA-256 and dimensions. Pixel materialization is
outside the timed region. The timed region starts immediately before a single
adapter CLI/API invocation and ends when that invocation returns. It includes
the wrapper/process overhead for every adapter, so results are comparable only
within this harness configuration.

All adapters are single-threaded by contract. Each image has the same 1000 ms
default wall-clock timeout. The report stores the pinned lock hash, observed
adapter version, hardware metadata, individual observations, p95 latency, and
a deterministic bootstrap 95% confidence interval for median latency.

The source-image and label annotation counts form the case manifest. The
reported `annotation_count_coverage` is merely returned-symbol count divided by
annotated symbol count; it is not localization recall or payload accuracy. Use
WP-002's quadrilateral/payload evaluator when a competitor adapter exposes the
required geometry and raw payload fields.

## Running

Build the exact pinned adapters from [`competitors/lock.json`](../competitors/lock.json)
into `competitors/bin/`. The lock includes sources, revisions, licenses, and
release build commands. Do not use a system binary with a different observed
version: the report returns `version_mismatch` and excludes it from timing.

```sh
python3 scripts/run_competitor_harness.py \
  --category nominal --limit 5 --max-dim 1024 --timeout-ms 1000 \
  --output artifacts/competitors/nominal.json
```

Select adapters explicitly during setup diagnostics:

```sh
python3 scripts/run_competitor_harness.py \
  --category monitor --limit 1 --adapter zbar --adapter opencv \
  --output /tmp/competitor-smoke.json
```

For a retained result, use `--require-verified-runner`. It prevents an
installation that merely reports a matching version from contributing empty
observations or timing samples:

```sh
python3 scripts/run_competitor_harness.py \
  --category monitor --limit 17 --adapter zbar --require-verified-runner \
  --max-dim 1024 --timeout-ms 1000 \
  --output artifacts/competitors/wp013-zbar-monitor-2026-07-13.json
```

Missing ZXing-C++, quirc, BoofCV, or rqrr wrappers are written as
`unavailable`, not removed from the report. The report contains no temporary
PGM paths, so it can be published alongside the exact lock and raw source
dataset fingerprint.

Every report also has a `setup` record per selected adapter. It says whether
the expected local runner was present and, when it was not, names the exact
missing executable or jar. This lets a setup-only smoke artifact document the
environment without incorrectly treating a missing adapter as a benchmark
result. The harness passes `OMP_NUM_THREADS`, `OPENBLAS_NUM_THREADS`,
`MKL_NUM_THREADS`, `VECLIB_MAXIMUM_THREADS`, and `NUMEXPR_NUM_THREADS` as `1`
to every adapter child process; those settings are recorded in the artifact.

## Local preflight

Use `--preflight` before materializing pixels when checking only whether local
adapter runners are present. It runs no image decode and emits a separate
`rustqr.competitor-preflight.v1` artifact:

```sh
python3 scripts/run_competitor_harness.py --preflight \
  --adapter zbar --adapter opencv \
  --output artifacts/competitors/wp013-local-preflight.json
```

The preflight records the lock hash, expected/observed version, runnable
command, and a minimal local capability check (`zbarimg --raw` and OpenCV
`QRCodeDetector`/`detectAndDecodeMulti`). A matching local version is reported
as `available_version_matches_unverified_provenance`, never as a pinned-build
verification: the check cannot prove source revision, compiler flags, wheel
origin, or transitive dependency configuration. Missing runners and mismatched
versions remain explicit statuses. A preflight is setup evidence only and is
not a competitor measurement or comparison claim.

On 2026-07-13, the local preflight artifact
`artifacts/competitors/wp013-local-preflight-2026-07-13.json` found ZBar
0.23.93 with `--raw` support and OpenCV 4.12.0 with both required detector
methods. Both remain provenance-unverified local installations.

The 2026-07-13 local smoke artifact at
`artifacts/competitors/wp013-local-smoke-2026-07-13.json` was generated with
all six adapters on `monitor/image001.jpg`. Its available pinned adapters were
ZBar 0.23.93 and OpenCV 4.12.0. ZXing-C++ needs
`competitors/bin/ZXingReader`; quirc needs `competitors/bin/quirc_decode`;
BoofCV needs `competitors/bin/boofcv_decode.jar`; and rqrr needs
`competitors/bin/rqrr_decode`. Those are build prerequisites, not negative
benchmark observations.

## Licenses and redistribution

| Adapter | Pinned license | Redistribution rule |
| --- | --- | --- |
| ZXing-C++ | Apache-2.0 | Preserve notices when redistributing binaries/source. |
| quirc | ISC | Preserve copyright and permission notice. |
| ZBar | LGPL-2.1-or-later | Do not ship an untracked system binary; satisfy LGPL source/relinking obligations if distributing one. |
| BoofCV | Apache-2.0 | Preserve notices when redistributing the runner/dependencies. |
| OpenCV | Apache-2.0 | Preserve notices when redistributing the selected wheel/library. |
| rqrr | MIT | Preserve the crate's copyright and permission notice. |

`competitors/` contains only locally authored adapters and version metadata;
it contains no competitor source, jar, wheel, or executable. Review the
upstream license files at the pinned revision before a release, since transitive
dependencies can add notices.

## Optional local runner verification

For a locally built file-based adapter, an operator may add an optional
`runner_sha256` field to that adapter's lock entry. Preflight promotes it to
`verified_pinned_runner` only when both the observed version and executable
SHA-256 match. The checked-in lock pins the locally installed ZBar 0.23.93
executable by SHA-256 (`50a0f12b…f3d155`) as a runnable, byte-identical local
runner. It remains a Homebrew-distributed LGPL binary: this digest proves the
runner used by the harness, not an independently reproducible upstream source
build or its transitive-library provenance. Other adapters remain
provenance-unverified until their listed pinned sources are built and their
resulting runner digests are recorded.

## Verified ZBar monitor observation (2026-07-13)

`artifacts/competitors/wp013-zbar-monitor-2026-07-13.json` records 17 monitor
images materialized once to the shared 1024px PGM contract and run through the
digest-verified ZBar 0.23.93 executable with the one-thread environment and a
1000ms per-image timeout. ZBar returned one payload line and no decode on the
other 16 images. The report's 1/17 annotation-count coverage is only a count
of returned lines versus quadrilateral annotations: these labels contain no
payload truth, and the ZBar CLI provides no localization output. It therefore
does not establish payload accuracy, localization recall, a RustQR comparison,
or a general ZBar accuracy/latency claim.

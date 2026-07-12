# Pinned competitor inputs

`lock.json` is the only version/build authority for WP-013. Third-party source
and binaries are intentionally not vendored or redistributed by this repository.
Build each adapter into `competitors/bin/` using the command recorded in the
lock, then verify its observed version in the harness artifact before comparing
results. A missing binary is an explicit `unavailable` result; a different
version is `version_mismatch` and is not benchmarked.

The adapters are thin newline-delimited payload CLIs. `quirc_decode.c`, the
`rqrr` crate, and the BoofCV Gradle runner are source-controlled wrappers; the
ZXing-C++ runner is the pinned upstream `ZXingReader` example and must be
copied into `competitors/bin/` after its build. Every runner writes one decoded
payload per line and supports `--version`.
Payload newlines are an unavoidable limitation of this common CLI transport and
must be called out for any binary-payload claim.

See `docs/competitor_harness.md` for the run contract and license constraints.

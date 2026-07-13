# Product-readiness boundaries

This document states the repository's current product-facing boundaries. It is
not a release announcement or a claim of platform/binding support.

## Versioning and compatibility

The package currently declares version `0.1.0` and MSRV Rust 1.85 in
`Cargo.toml`. Until a published release policy and compatibility suite exist,
the `0.x` API is experimental: public APIs may change in a minor release and
consumers should pin an appropriate version range. The supported Rust API is
the documented public surface in `src/lib.rs`; `qrtool` requires the optional
`tools` feature.

Before declaring a stable `1.0` API, the project needs a published changelog,
release/tag procedure, compatibility policy, and fixture-sharing plan for any
bindings. No crates.io publication or binding compatibility is claimed here.

## Security reporting and maintenance limits

This repository has deterministic malformed-input tests and a scheduled
sanitizer fuzz workflow, but no published response-time commitment or separate
security contact in the tracked project files. Do not include credentials,
private datasets, or exploit material in a public issue. Use the repository's
private GitHub security reporting facility if it is enabled; otherwise contact
the repository owner through GitHub before disclosing sensitive details.

The fuzz and adversarial suites improve crash/input-safety coverage. They do
not establish a security guarantee, a production false-positive budget, or a
supported-security-window commitment.

## Dataset and benchmark provenance

The checked-in BoofCV benchmark tree is annotation-based. Its image and label
fingerprints are emitted in reading-rate artifacts, and the evaluator records
resize policy, selected category, commit, and matching semantics. Those
fingerprints establish repeatability of a run against the local tree; they do
not establish redistribution rights or the provenance of every external image.

`tests/negative_corpus` is different: it is a self-authored deterministic
renderer with an `MIT OR Apache-2.0` manifest, as documented in
`docs/wp009_adversarial.md`. Its thirteen synthetic images are not photographs
or valid examples of third-party barcode formats. Published benchmark or
release material must identify the dataset source, revision, license, and
redistribution limits separately.

## Current support boundary

CI covers hosted Linux, macOS, and Windows library tests; Rust 1.85 runs the
all-features test suite. The crate remains `std`-based. WASM, iOS, Android,
`no_std`, and language bindings are not supported until dedicated build/test
lanes and compatibility evidence exist.

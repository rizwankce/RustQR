# WP-009 adversarial safety evidence

This document records the safety evidence that is available now and, equally
important, the false-positive evidence that is not available yet.

## Deterministic checks

- `tests/adversarial_matrix_tests.rs` rejects seeded QR-sized noise, malformed
  dimensions, and finder/timing damage beyond the decoder recovery tolerance.
- The existing structural-invalid conformance fixtures cover invalid format and
  version BCH words, non-zero remainder bits, and invalid payload padding.
- `tests/input_api_tests.rs` checks public image-input validation for grayscale,
  RGB, RGBA, padding, short buffers, invalid strides, zero dimensions, and
  arithmetic overflow.
- `fuzz/public_image_input` and `fuzz/matrix_decode` cover the public image
  boundary and deterministic matrix decoder respectively. The latter reaches
  format/version parsing, deinterleaving, Reed-Solomon correction, payload
  parsing, and erasure-sidecar validation when arbitrary input survives the
  earlier gates.

## False-positive metric limitation

**No image-level or per-megapixel false-positive rate is reported yet.** This
repository does not yet contain a licensed, annotated negative-image corpus
covering text, checkerboards, packaging, screens, Data Matrix, Aztec, linear
barcodes, finder-like graphics, and random noise. Matrix rejection fixtures
measure decoder safety only; they do not measure detector false positives.
Likewise, fuzzing establishes crash/memory-safety coverage, not a false-positive
rate. No recovery change may claim compliance with a false-positive budget until
that corpus, an image-level evaluator, and an agreed budget are added.

## Reproducing the local checks

```sh
cargo test --test adversarial_matrix_tests --all-features
cargo test --test input_api_tests --all-features
cargo fuzz run public_image_input -- -max_total_time=30
cargo fuzz run matrix_decode -- -max_total_time=30
```

The two fuzz commands require the `cargo-fuzz` tool and a sanitizer-capable
host. CI runs a bounded sanitizer pass; developers should run substantially
longer campaigns before security-sensitive releases.

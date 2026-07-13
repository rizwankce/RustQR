# WP-009 adversarial safety evidence

This document records the safety evidence that is available now and the scope
limits of its false-positive measurements.

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

## Deterministic negative-image corpus

`tests/negative_corpus/manifest.json` declares a thirteen-image synthetic
corpus (256 by 256 or 128 by 128 pixels). Its images are rendered deterministically by
`tests/negative_image_corpus_tests.rs`, which is the corpus asset: no opaque
binary image or third-party material is included. The manifest and renderer are
licensed `MIT OR Apache-2.0` and self-authored by this repository.

The current generators cover these *synthetic approximations*:

- block text, checkerboard, package-panel, and screen-grid graphics;
- Data-Matrix-like border/data pattern, Aztec-like concentric target, and
  linear-barcode-like bars;
- isolated finder-like targets with inconsistent placement; and
- seeded black/white noise.
- source-generated halftone dots, crossed moire-like stripes, inconsistent
  nested square rings, and a QR-corner finder triplet with intentionally broken
  timing/data structure.

They do **not** claim to be photographs, screenshots, commercial packaging, or
valid Data Matrix/Aztec/barcode examples. Those external categories still need
appropriately licensed, annotated material before a production false-positive
budget can be accepted.

The evaluator runs the public grayscale image API and counts every returned QR
object as a false-positive detection. The expanded corpus contains 655,360
pixels (0.655360 megapixels). The four added adversarial cases each have a
focused public-API regression asserting zero detections; the full evaluator is
the authoritative command for emitting the aggregate JSON report. The corpus
is deliberately synthetic, so these metrics are bounded to the thirteen
generated images rather than an estimate of production FPR.

| Metric | Result |
| --- | ---: |
| positive images | 0 |
| false-positive detections | 0 |
| false positives per image | 0.0 |
| false positives per megapixel | 0.0 |

This is a reproducible synthetic-corpus baseline, not a general detector FPR
claim. Matrix rejection fixtures measure decoder safety only, and fuzzing
establishes crash/memory-safety coverage rather than a false-positive rate.
No recovery change may claim compliance with a production false-positive budget
until a broader annotated corpus and an agreed budget are added.

## Reproducing the local checks

```sh
cargo test --test adversarial_matrix_tests --all-features
cargo test --test input_api_tests --all-features
python3 scripts/evaluate_negative_corpus.py --output /tmp/negative-corpus.json
cargo fuzz run public_image_input -- -max_total_time=30
cargo fuzz run matrix_decode -- -max_total_time=30
```

The two fuzz commands require the `cargo-fuzz` tool and a sanitizer-capable
host. CI runs a bounded sanitizer pass; developers should run substantially
longer campaigns before security-sensitive releases.

## Fuzz campaign replay evidence

The scheduled/manual AddressSanitizer workflow runs each target with a bounded
`-max_total_time` (120 seconds by default). It uses `pipefail` and uploads a
per-target sanitizer log regardless of outcome. On a failure it separately
uploads the minimized input directory. The log establishes the exact remote
campaign invocation; the minimized input is the artifact to replay locally.
This improves reproducibility of remote-only fuzz validation but does not
claim that a local sanitizer campaign ran in this network-restricted workspace.

## Bounded synthetic evaluation

The synthetic corpus integration test evaluates each image through
`try_detect_with_options` with diagnostics and a five-second request-scoped
deadline. A timeout is emitted as `timeout_images` in
`NEGATIVE_CORPUS_METRICS` and causes the test to fail; it is not folded into
the zero-detection numerator. `scripts/evaluate_negative_corpus.py` carries
that field into its JSON report separately from false-positive counts.

The focused local run completed all thirteen generated cases with
`timeout_images=0` and zero detections. The limit is cooperative, so it bounds
new request work but is not a process-level hard timeout. This improves the
repeatability of the synthetic evaluator only and does not change its
synthetic-only FPR scope.

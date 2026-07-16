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

## External-corpus admission gate

The checked-in `benches/images/boofcv` tree is **not** a WP-009 candidate. Its
local README names upstream benchmark sources, but the tree contains neither an
image-level license nor a redistribution grant, and its annotations describe
positive QR symbols rather than negative-image ground truth. Do not relabel,
copy, or derive a negative corpus from it without independently recording that
authority.

The concrete current path is therefore two-stage:

1. keep `tests/negative_corpus` as the only locally admissible, self-authored
   baseline; and
2. add a separately sourced external corpus only with a tracked manifest that
   records, for every asset, its source URL, immutable revision or download
   date, license text/identifier, redistribution constraint, SHA-256, category,
   dimensions, and an explicit `expected_qr_count: 0` annotation.

Before enabling that manifest as a merge gate, define the image and megapixel
false-positive budgets, decide whether failures include timeouts, and run the
same public-API evaluator used by the synthetic corpus. A manifest missing any
of those provenance fields remains an intake record, not safety evidence.

### Admitted slice: Wikimedia Commons photographed screen

`wikimedia_commons/computer-screen-monitor.jpg` is the unmodified original of
[`Computer Screen Monitor.jpg`](https://commons.wikimedia.org/w/index.php?title=File:Computer_Screen_Monitor.jpg&oldid=1131317639), authored by U3211603 and
licensed CC BY-SA 4.0. Its manifest pins the permanent Commons revision,
original URL, Commons SHA-1, local SHA-256, dimensions, and a manual original-
image visual annotation that no QR symbols are present. The accompanying
license reference, notice, and REUSE declaration preserve the attribution and
license link needed when redistributing the fixture.

The strict public-RGB release evaluator completed this 4624 by 3468 image
(16.036032 MP) with zero returned QR objects and no five-second cooperative
timeout in 2.88 seconds. Its diagnostic terminal stage was Reed-Solomon
rejection, not a timeout. An unoptimized debug build does time out under that
same cooperative deadline, so this is an ignored release-qualification test;
it is not part of the normal debug test suite. Run
`python3 scripts/evaluate_negative_corpus.py --include-wikimedia` to verify its
hash/dimensions and execute the strict release gate.

This is one photographed-screen category example, not a license to infer that
all Commons assets are QR-free, a production FPR estimate, or coverage for the
still-missing photographed text and packaging categories.

### Admitted slice: ZXing negative black-box images

The `falsepositives` (22 PNGs) and `falsepositives-2` (25 PNGs) directories
from ZXing are now vendored under `tests/negative_corpus/`, with one immutable
per-asset record in `zxing_manifest.json`. The source is
[`zxing/zxing`](https://github.com/zxing/zxing), pinned to commit
[`82333b3ed894ef097d41dd8c922689ede8880e01`](https://github.com/zxing/zxing/tree/82333b3ed894ef097d41dd8c922689ede8880e01)
(commit date `2026-07-11T19:40:42-05:00`). The immutable source paths are:

- `core/src/test/resources/blackbox/falsepositives/{01..22}.png`; and
- `core/src/test/resources/blackbox/falsepositives-2/{01..25}.png`.

The upstream REUSE declaration, `.reuse/dep5`, assigns `Apache-2.0` to
`Files: *` with only listed exceptions outside these paths. Its
[`LICENSE`](https://github.com/zxing/zxing/blob/82333b3ed894ef097d41dd8c922689ede8880e01/LICENSE)
grants reproduction and distribution in source or object form, including with
modifications, subject to retaining the license and relevant notices. The
upstream `NOTICE` contains only Barcode4J and JCommander notices, neither of
which applies to these image paths. The vendored slice retains the Apache-2.0
license, complete upstream NOTICE, and `.reuse/dep5` as
`tests/negative_corpus/licenses/ZXING-*` files.

The negative annotation is source-authored rather than inferred from an image
search: at the same commit,
`FalsePositivesBlackBoxTestCase` describes its images as random high-contrast
patterns that “do not decode as barcodes,” and
`FalsePositives2BlackBoxTestCase` describes additional such images that “should
not find any barcodes.” Their common `AbstractNegativeBlackBoxTestCase` tests
for a failure to decode every image. Therefore each selected asset may be
entered as `expected_qr_count: 0` after a RustQR-side visual and decoder audit.
The legacy ZXing test allows a small aggregate decoder false-positive tolerance;
that is a tolerance of its implementation, not a claim that any source image
contains a QR code.

`scripts/verify_wp009_zxing_corpus.py` verifies every local asset's SHA-256,
PNG dimensions, unique path/id, expected zero QR count, upstream path, and
the required provenance files before `scripts/evaluate_negative_corpus.py`
runs the public API evaluator. The manifest records each local/source path,
SHA-256, dimensions, category, and `expected_qr_count: 0`.

The release evaluator completed all 47 assets (12.518400 MP) with zero
timeouts, zero positive images, and zero returned QR objects. That is a
reproducible result for this licensed high-contrast slice only. It is **not** a
production FPR claim: it has no photographed text, packaging, screens, or
representative valid non-QR symbologies, and it does not define the still-open
production false-positive budget.

## Reproducing the local checks

```sh
cargo test --test adversarial_matrix_tests --all-features
cargo test --test input_api_tests --all-features
python3 scripts/evaluate_negative_corpus.py --output /tmp/negative-corpus.json
python3 scripts/verify_wp009_zxing_corpus.py
python3 scripts/verify_wp009_wikimedia_corpus.py
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

The same timeout/FPR semantics apply to the admitted ZXing slice: every input
is decoded through `try_detect_with_options` using RGB bytes and a five-second
request-scoped deadline. A `FailureStage::Timeout` fails the test and remains a
separate `timeout_images` value; it is never folded into the zero-detection
numerator. `ZXING_NEGATIVE_CORPUS_METRICS` reports its own image and megapixel
denominators. The optional `WP009_EXTERNAL_SHARD=INDEX/COUNT` test setting is
only for resource-constrained verification; without it the test evaluates all
47 assets.

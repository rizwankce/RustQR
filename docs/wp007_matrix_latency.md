# WP-007 matrix-decode latency evidence

This is a controlled component measurement for the spec-first decoder. It is
not a photographic detection, reading-rate, or end-to-end latency claim.

## Workload and timing boundary

`benches/matrix_decode.rs` parses the checked-in, clean, correctly oriented
Version 1-M numeric matrix at
`tests/conformance/fixtures/golden-v1-m.matrix`. It verifies the expected raw
payload (`4376471154038`) once, then Criterion repeatedly measures the public
`QrDecoder::decode_matrix(&matrix, 1)` call. Matrix construction, fixture
parsing, image loading, detector work, and assertion setup are outside the
timed closure. The call includes the matrix API's dimension and format checks,
structural validation, canonical traversal, deinterleaving, Reed-Solomon, and
payload decoding.

The fixture is deliberately clean and canonical. It supplies the common-case
path evidence, not damaged-symbol recovery evidence.

## Reproduction

```bash
cargo fmt -- --check
cargo bench --bench matrix_decode
```

Criterion stores machine-local comparison history under `target/criterion/`.
For a deliberate same-machine comparison, run the command once before a
decoder change and once after it without deleting that directory, and retain
the Criterion output with the commit IDs and environment details.

## Matched pre/post measurement (2026-07-12)

Command:

```text
cargo bench --bench matrix_decode
```

Both revisions were extracted with `git archive` into separate temporary
directories, so neither benchmark changed the active worktree. The older
revision did not contain the benchmark target, therefore only its temporary
copy received the byte-for-byte benchmark source and Cargo target registration
from the current revision. No decoder source was changed. Both copies used the
same fixture SHA-256:
`966a96def275227273000131b513b01a568f5b2baee672f9c34481eee8fb13e5`.

Result (Criterion 100 samples, 95% confidence interval; second alternating
run for each snapshot):

| Comparison point | Revision | Benchmark | Time |
|---|---|---|---|
| Before the spec-first slice | `82fc5172ad1c19bf6ba245302e93bbb56a151782` | `decode_matrix/canonical_v1_m_numeric` | 6.9570–6.9782 us |
| Current measured snapshot | `e41cb144ef4f1e562c54a7724bc7943db8d52ac4` | `decode_matrix/canonical_v1_m_numeric` | 6.3762–6.3959 us |

Environment: Apple Silicon (`aarch64-apple-darwin`), macOS Darwin 25.5.0,
Rust 1.85.0, release Criterion benchmark. Gnuplot was unavailable, so
Criterion used its Plotters backend; that does not change the timing samples.

## Before/after assessment

The matched point estimates are 6.9673 us before and 6.3859 us after, an
8.35% reduction for this clean canonical workload. A first alternating pair
was noisier but had the same direction (6.9501–7.4565 us before and
6.5084–6.5992 us after). This replaces the previous "not measured" statement:
the pre-WP-007 implementation can be measured under the same timing boundary.

This does **not** complete the latency acceptance criterion. The roadmap does
not define a threshold for "drops substantially," and an 8.35% component
reduction cannot defensibly be called substantial without one. It is evidence
of a modest clean-matrix improvement only; it is not a photographic detection,
reading-rate, or end-to-end claim.

The separate targeted photographic regression run remains the recall evidence
for recovery behavior. A future WP-007 completion claim needs a defined
substantial-latency threshold (and evidence against it) plus the existing
targeted recovery checks.

## Current-state audit (2026-07-13)

No repository roadmap, benchmark document, or CI configuration defines a
numeric (or otherwise objectively evaluable) threshold for the acceptance
phrase “latency drops substantially.” Consequently, the matched archive result
above remains a modest component improvement, not completion evidence.

At commit `47319a4684bbfcff00adcf2f10565f07ddb242cf`, two immediate local
runs of the same command gave the following 95% confidence intervals:

| Run | Benchmark | Time |
|---|---|---|
| 1 | `decode_matrix/canonical_v1_m_numeric` | 9.2808–10.071 us |
| 2 | `decode_matrix/canonical_v1_m_numeric` | 8.7282–9.1043 us |

These were shared-machine measurements, not isolated `git archive` snapshots.
Their spread means they are recorded only as the current local range; they do
not support a direction or regression claim against the 2026-07-12 archive
pair.

The documented `QR_MAX_DIM=800` ignored photographic command completed all
seven functions at this revision. Monitor and rotated produced decodes, and
the multi-code smoke produced five results. Blurred, nominal, damaged, and
high-version cases emitted their allowed no-decode warnings. Inspection of
`tests/decode_regression_tests.rs` confirms that these tests accept an empty
result for most cases and do not assert expected payloads or expected
multi-code counts. They are therefore smoke/safety evidence, not a
no-recall-loss acceptance gate. A WP-007 completion claim requires a defined
latency threshold plus a payload- and count-asserting photographic recall
comparison.

## Photographic gate scope (2026-07-13)

`tests/decode_regression_tests.rs` now contains two strict, ignored
photographic gates instead of warning-only pass cases:

```sh
cargo test --test decode_regression_tests \
  --release --all-features -- --ignored
```

They fix the resize policy at an 800px maximum dimension. The monitor fixture
asserts its raw/text payload, version, EC level, one expected result, and a
one-to-one IoU >= 0.5 geometry match against its BoofCV label. The close
fixture asserts its independently observed raw/text payload, Model 2 version
7, EC-L, one expected result, and the same one-to-one geometry condition with
no false positives or duplicates. The other historical image001 fixtures are
not declared passing regressions: their labels establish expected counts
(blurred 1, high-version 1, rotations 3, damaged 1, lots 60, nominal 2), while
current known misses remain explicitly outside the strict acceptance set. The
labels do not contain payloads, so a full photographic recall gate still needs
independently sourced payload data as well as count and geometry assertions.

The same two fixtures also have focused ignored diagnostic tests. They prove
that their successful request-scoped paths require zero matrix recovery-mode
attempts and zero RS-erasure attempts at that fixed resize policy. This is
evidence that the two strict successes do not rely on brute-force decoder
recovery; it is not category-wide recall evidence.

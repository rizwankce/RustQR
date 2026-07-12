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

## Current baseline (2026-07-12)

Command:

```text
cargo bench --bench matrix_decode
```

Result (Criterion 100 samples, 95% confidence interval):

| Comparison point | Revision | Benchmark | Time |
|---|---|---|---|
| Before the spec-first slice | No equivalent measurement | `decode_matrix/canonical_v1_m_numeric` | Not measured |
| After the measurement-only evidence change / current baseline | `72ac730` plus uncommitted roadmap work | `decode_matrix/canonical_v1_m_numeric` | 6.9299–7.0763 us |

Environment: Apple Silicon (`aarch64-apple-darwin`), macOS Darwin 25.5.0,
Rust 1.85.0, release Criterion benchmark. Gnuplot was unavailable, so
Criterion used its Plotters backend; that does not change the timing samples.

## Before/after assessment

There is no valid pre-WP-007 measurement: the prior revision did not contain
this fixed-fixture benchmark or a recorded equivalent timing boundary. A
historical before number must therefore remain **not measured**, rather than
being inferred from unrelated detector or reading-rate timings. This evidence
establishes the current baseline and a reproducible comparison method, but it
does not prove the acceptance criterion that latency *dropped substantially*.

The separate targeted photographic regression run remains the recall evidence
for recovery behavior. A future WP-007 completion claim needs a matched
pre/post Criterion comparison and the same targeted recovery checks.

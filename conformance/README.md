# Model 2 conformance corpus

`manifest.json` is a deterministic foundation manifest, generated with:

```bash
python3 scripts/generate_conformance_manifest.py \
  --output conformance/manifest.json
```

It records stable seeds, expected raw payload bytes, version, error-correction
level, mask, and mode metadata. Matrix paths remain `null` and their status is
`planned` until an independent encoder adapter materializes them. This avoids
checking in synthetic matrices whose provenance cannot be verified.

Use `--full` to generate the 40-version, four-EC-level, eight-mask cross-product
with all mode scaffolds. The foundation manifest is intentionally smaller for
review and CI. Capacity-boundary, error/erasure, invalid-matrix, and
differential-decoder variants will extend the same schema rather than overwrite
existing case identities.

The full supported gate generates its 3,840 numeric, alphanumeric, and byte
matrices in a temporary directory, checks their recorded checksums, and asserts
the exact payload, Model 2 version, EC level, and mask for every decoded case:

```bash
cargo test --test conformance_matrix_tests --all-features \
  -- --ignored generated_full_supported_corpus_reaches_one_hundred_percent
```

It requires the pinned `qrcode==8.2` package from `requirements.txt`; the
generated images are deleted after the test and are not checked in.

Supported matrices are materialized offline with the pinned mature generator
already recorded in `requirements.txt`:

```bash
python3 scripts/materialize_conformance_matrices.py \
  --manifest conformance/manifest.json
```

The backend fixes Model 2 version, EC level, and mask. It generates numeric,
alphanumeric, and byte cases with `python-qrcode==8.2`, and materializes the
four compact header-mode fixtures through a small ISO segment adapter. That
adapter writes Kanji, ECI, FNC1, and Structured Append header bits directly,
then delegates RS coding and module placement to the pinned backend. It records
the PNG SHA-256, symbol dimension, quiet zone, pixel scale, and backend version.
It also binary-searches the encoder's maximum capacity for each materialized
version/EC/mode tuple and records that the maximum is accepted and
maximum-plus-one is rejected.

Run the independent decoder adapters and reproduce `differential-report.json`
with:

```bash
python3 scripts/run_differential_decoders.py \
  --manifest conformance/manifest.json \
  --output conformance/differential-report.json
```

The report verifies each generated PNG checksum before invoking an adapter and
accounts separately for matches, mismatches, unavailable engines, decode
failures, and explicitly unsupported fixtures. With ZBar 0.23.93 and OpenCV
4.12.0, ZBar matches all 30 generated text fixtures; OpenCV matches 25 and
reports five decode failures. Neither adapter reports a payload mismatch. The
four unsupported-mode scaffolds are excluded from generated-fixture totals.

Kanji payload bytes are Shift-JIS. The decoder records ECI assignment numbers,
GS1/FNC1 position (including the second-position application indicator), and
Structured Append sequence metadata. The compact corpus contains one valid
matrix for each of Kanji, ECI, GS1/FNC1, and Structured Append; its integration
test asserts the exact raw payload plus public metadata. Header fixtures are
intentionally not included in the 3,840-case full supported-mode grid, which
continues to measure numeric, alphanumeric, and byte coverage only.

`mutation_plan.json` records deterministic corruption intent separately from
the generated matrices. Regenerate it with:

```bash
python3 scripts/plan_conformance_mutations.py \
  --manifest conformance/manifest.json \
  --output conformance/mutation_plan.json
```

Correctable error counts use `floor(ecc_codewords_per_block / 2)` and erasure
counts use all ECC codewords per block from `src/decoder/tables.rs`. The plan
pins both manifest and table hashes. Entries remain `planned` until a matrix
backend provides the named codeword/module, RS-block, or confidence maps;
planned entries are never counted as decoder successes. Invalid format,
version, remainder, padding, and block-layout cases similarly expect rejection
only after their required maps exist. Version-information corruption is
explicitly `not_applicable` for versions below 7.

`scripts/qr_spec_mapping.py` provides the encoder-independent Model 2 data
placement traversal, complete-codeword module groups, remainder modules, and
raw-codeword-to-RS-block mapping needed by the mutation materializer. Its tests
cover published symbol counts, placement order, and mixed short/long RS block
groups. Actual mutations remain planned until the materializer records changed
modules and, for erasures, a decoder-consumable confidence sidecar.

Run `scripts/materialize_conformance_mutations.py` to create mapping-backed
correctable-error, correctable-erasure, and invalid-block-layout variants.
Every artifact records its seed, selected raw codewords, changed module
coordinates, and checksum. Erasure variants record a sparse confidence sidecar
and are executable through `decode_matrix_with_erasures`; the conformance tests
assert the expected payload at each fixture's full per-block erasure boundary.
Single-block symbols mark invalid block layout as `not_applicable`. For every
multi-block symbol, the materializer deterministically searches all RS block
pairs for unequal codeword swaps, then records the affected blocks, the number
of resulting errors per block, and the correction limit. The selected count is
always one above that limit, so a materialized block-layout fixture is a
provable rejection case rather than an ambiguous corruption.

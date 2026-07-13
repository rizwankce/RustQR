# Conformance differential adapters

WP-006 uses `scripts/run_differential_decoders.py` to compare matrix-backed
manifest cases with two independent decoder engines already available in the
local development environment:

| Adapter | Local engine | Result fidelity | Known limitation |
| --- | --- | --- | --- |
| `zbar` | ZBar `zbarimg` | Text payload | `--raw` still recodes arbitrary binary QR payloads |
| `opencv` | OpenCV `QRCodeDetector` | UTF-8 text re-encoded as bytes | Cannot verify arbitrary non-UTF-8 byte or Kanji/ECI raw-byte fixtures |

`pyzbar` is not a third independent decoder because it uses the same ZBar
engine. `zxingcpp` is not installed locally, so it is not part of the
reproducible offline lane.

Run both adapters with:

```sh
python3 scripts/run_differential_decoders.py \
  --manifest conformance/manifest.json \
  --output conformance/differential-report.json
```

The report records the manifest SHA-256, adapter versions, exact raw payload
hex when available, and a status for every case. Missing generated matrices,
unavailable engines, unsupported binary comparisons, timeouts, decode errors,
and payload mismatches are distinct states. The adapter does not rewrite the
generation manifest or its stable case identities.

For a materialized invalid case, set `expected.outcome` to `reject`. A decoder
failure then becomes `expected_rejection`, while any returned payload becomes
`unexpected_accept`. A plain `decode_error` is never silently counted as a
valid-fixture success.

## Current RustQR support gaps

The payload parser handles numeric, alphanumeric, byte, and Kanji segments. It
preserves Kanji Shift-JIS bytes but converts display text lossily. It records
ECI assignment numbers, GS1/FNC1 position, and Structured Append sequence
metadata, but intentionally does not apply ECI character-set conversion or
reassemble Structured Append symbols. The compact corpus has end-to-end
matrices for these headers; adapters that return only text still cannot verify
non-UTF-8 raw bytes or RustQR metadata fields.

The public matrix decoder supports the materialized header modes above and
reports any remaining unrecognized mode explicitly. The conformance harness
treats a decoder failure as a valid outcome only for an artifact whose manifest
expectation is rejection.

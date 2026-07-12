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

The present payload parser handles numeric, alphanumeric, and byte segments.
It reconstructs Kanji Shift-JIS bytes but converts display text lossily and has
no dedicated conformance coverage. It consumes ECI assignment numbers but
does not apply the selected character encoding. GS1/FNC1 and Structured Append
mode indicators return a generic decode failure. `QRCode` also has no segment,
ECI, GS1, or Structured Append metadata fields, so those features cannot yet
be differentially asserted beyond raw payload behavior.

Until WP-007 introduces explicit decoder errors, unsupported modes are not
distinguishable from corrupt matrices through the public matrix decoder. The
conformance harness must therefore classify those manifest cases as expected
unsupported rather than treating a generic failure as proof of correct
feature handling.

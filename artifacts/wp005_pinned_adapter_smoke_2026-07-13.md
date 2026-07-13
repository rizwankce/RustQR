# WP-005 pinned adapter smoke

Date: 2026-07-13

This is an adapter and shared-normalizer smoke only. It is not an accuracy,
latency, or branch-selection result.

## Inputs

- Main source: `5b9b41e805ab487683aa1f8af986438377ff04bf`
- Rebuild source: `295b97c524a7c9133a1fba02b3598aa60388474f`
- Fixed slice: `nominal/image001.jpg`
- Limit: one image per category
- Preprocessing: `rgb8;triangle-resize;max-dim=1024`
- Timeout: zero (the historical public APIs do not expose a compatible
  request-timeout interface)

## Evidence

Both throwaway binaries built with:

```sh
cargo build --release --features tools --bin wp005_prediction_export
```

Both exported one `rustqr.wp005.prediction-stream.v1` row with identical image
ID and `fnv1a64:d684f3ec5eccd229` dataset fingerprint. A shared truth manifest
from the exact selected image list then successfully normalized both exports
using `scripts/normalize_wp005_prediction_stream.py`.

The normalized localization result was 0/2 labels for each source. Rebuild
reported no detections. Main decoded a payload but exposed four zero-valued
position points; the adapter drops that invalid payload/geometry pair rather
than writing a zero-area quadrilateral. Therefore this proves that both
historical source APIs can serialize and normalize valid streams, not that
either branch has comparable accuracy or latency.

## Next gate

Run the documented seven-category slice at 25 images per category from the
same fixed input list. Main needs a geometry-producing export path before a
localization comparison can be considered complete.

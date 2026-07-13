# WP-011 bounded category probe

## Scope

This is a reproducible, one-image-per-category diagnostic at the current
committed revision `47319a4`, not a category acceptance result or a
before/after comparison. Every command used the BoofCV dataset,
`QR_MAX_DIM=800`, `--limit 1`, and the CLI's 2,500 ms cooperative deadline:

```sh
QR_MAX_DIM=800 target/release/qrtool reading-rate \
  --category <category> --limit 1 --non-interactive --timeout-ms 2500 \
  --artifact-json artifacts/wp011_category_<category>_qrmax800_limit1_2500.json
```

Each artifact retains its raw runtime, score, timeout, and stage telemetry.
The first image is not necessarily a one-symbol image, so scores below are
symbols matched / localization labels for one image.

## Results

| Category | Match | Timeout | Core / end-to-end | Attempts | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| perspective | 0/1 | 0 | 1879.82 / 1919.41 ms | 33 | Decode miss, before deadline. |
| rotations | 2/3 | 0 | 216.25 / 267.28 ms | 3 | Partial successful route. |
| brightness | 2/3 | 0 | 170.49 / 225.88 ms | 3 | Partial successful route. |
| bright_spots | 0/3 | 0 | 393.66 / 613.33 ms | 23 | Decode miss, before deadline. |
| glare | 0/1 | 1 | 2607.54 / 2790.12 ms | 39 | Late result discarded by cooperative deadline. |
| shadows | 2/3 | 0 | 497.80 / 598.10 ms | 7 | Partial successful route. |
| curved | 0/1 | 0 | 1395.48 / 1407.06 ms | 54 | Decode miss; both binarization fallback transitions ran. |

All seven artifacts report zero false positives and zero duplicate predictions.
They do not demonstrate category improvements: there is no matched baseline,
the sample contains just one image per category, and most required categories
still have a miss. The timeout in glare also confirms that the current request
deadline remains cooperative, rather than a hard interruption of an in-flight
scan or decode.

The diagnostic is sufficient to establish the next evidence boundary: retain
the 800-pixel setting and deadline, add an explicit external wall-clock guard
only when hard-cap evidence is required, and collect a fixed multi-image list
only after a proposed geometry/sampling change is ready for a matched
comparison. The curved mesh remains unproven because this run produced no
successful curved decode.

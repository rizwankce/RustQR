# WP-005 matched macOS Fast Benchmark runs

The local shared-v2 comparison remains the accuracy decision evidence. These
Actions runs prove that both historical implementations completed the required
macOS, 1024-pixel, seven-category Fast Benchmark configuration. Their output
schemas differ and must not be used as a direct accuracy delta.

## Runs and provenance

| Candidate | Run | Ref | Workflow provenance |
| --- | --- | --- | --- |
| main | [29241027260](https://github.com/rizwankce/RustQR/actions/runs/29241027260) | `5b9b41e805ab487683aa1f8af986438377ff04bf` | native Fast Benchmark workflow |
| rebuild | [29240862028](https://github.com/rizwankce/RustQR/actions/runs/29240862028) | `295b97c524a7c9133a1fba02b3598aa60388474f` | wrapper `3f2ab45`, historical source plus workflow-only CLI compatibility |

Both successful runs used `platform=macos`, `bench_limit=25`, the categories
`nominal,rotations,perspective,high_version,brightness,bright_spots,lots`, and
the workflow's fixed `QR_MAX_DIM=1024`. `lots` has seven available images.

## Results

| Category | Main label hits / labels | Main median ms | Rebuild image matches / cases | Rebuild median ms |
| --- | ---: | ---: | ---: | ---: |
| nominal | 21 / 29 | 756.714 | 18 / 25 | 126.521 |
| rotations | 2 / 76 | 554.177 | 13 / 25 | 162.572 |
| perspective | 12 / 25 | 407.098 | 20 / 25 | 128.733 |
| high_version | 0 / 25 | 2446.426 | 1 / 25 | 187.547 |
| brightness | 6 / 76 | 731.533 | 6 / 25 | 237.915 |
| bright_spots | 0 / 76 | 395.823 | 0 / 25 | 230.092 |
| lots | 1 / 420 | 413.820 | 0 / 7 | 240.027 |

Main emitted `rustqr.reading_rate.v1`, including per-label count-based rates.
Rebuild emitted `wp007-reading-rate-v1`, where a case is successful when any
code is returned. The schemas, count units, and match semantics are
incompatible; the table is configuration evidence only. The normalised local
v2 artifact pair is the sole branch-selection comparison.

## Downloaded artifact SHA-256

The following downloaded JSON checksums identify the exact retained Actions
artifacts while GitHub keeps them available:

| Category | Main SHA-256 | Rebuild SHA-256 |
| --- | --- | --- |
| nominal | `001edfa845e58641bca98e95190d33d29ea0574391787dc8173cf94cecce4625` | `10bce9973108b700206e982fa0854563bf4eed0841abab1485f8795f7d729fec` |
| rotations | `37a9a5ad78edb4c3f4a969675b2bcecf0344765cab8caf9a0a103d43f33ef076` | `1c526e6e90ae2eead23e0076b8b6de69d9735d51d2bca51ee7b1f8b51b72219d` |
| perspective | `25facb4fa3a044875429eb463afbc793079e90c33c78de2c7de0eb7e1373cda2` | `82831d498d118855e3f7cc61d091d4dc386d99f48ced33a00b466f499fa366d1` |
| high_version | `1a5433dc02349e5413caecd4fba69fdff9b7aa9bc53bf28dff0b4e0811718ce8` | `7959d53dc5e2115dcf9599087036dd7d1af6904599a4ddfa71e4a5a2ce39e713` |
| brightness | `2c2160b798ebf70ff9d17ed1c6bb477dc21ccbef424a93f47bd44eb5fae3a6ca` | `75a0cad1fa000dc5ca1bd04ef24952eb264b8d67b0e1f43be4b3777868c7c0e6` |
| bright_spots | `0c178be39748068dc792ba5121db0e59d8e3ad5e85ed167383b45a1db98dfcb7` | `e9e5c45ec02240418a1a4b62f8df25d6ca8800187314fb85f1f6859e0e65ce9c` |
| lots | `9d47db4c817790838952926f1497f93c5aa0256f7ec647031c4d817faba5ab4e` | `423e52c475794f1f92ce9035e83b01d0a94e5a47dccf6cf252c635f20638f985` |

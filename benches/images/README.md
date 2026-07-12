# Benchmark Test Images

Benchmark datasets live under this folder.

## Supported Formats
- PNG
- JPG/JPEG
- GIF
- BMP

## Image processing

- Images may be downscaled according to `QR_MAX_DIM` (1024 in benchmark
  workflows); set `QR_MAX_DIM=0` to preserve original resolution.
- Multiple-symbol inputs are accepted, but current scoring and decode coverage
  are partial; see `TODO.md` WP-002 and `docs/spec.md`.

## Layout
- `boofcv/` - BoofCV QR benchmark dataset (16 categories)
- `custom/` - your own images for quick experiments

## Benchmarking
Run Criterion benchmarks with:
`cargo bench --features tools --bench real_qr_images`

Run the end-to-end reading-rate tool with:
`cargo run --features tools --bin qrtool --release -- reading-rate --limit 3`

Optional environment variables:
- `QR_DATASET_ROOT` (default: `benches/images/boofcv`)
- `QR_BENCH_LIMIT` (default: full dataset; set a positive value to limit, or `0` for full)
- `QR_SMOKE` (set to `1` to use `_smoke.txt` inside the dataset root)
- `QR_MAX_DIM` (workflow default: `1024`; `0` preserves original resolution)

The CLI `--limit` overrides `QR_BENCH_LIMIT`.

## Sources
Test images can be downloaded from:
- Dynamsoft QR Benchmark: https://www.dynamsoft.com/codepool/qr-code-reading-benchmark-and-comparison.html
- BoofCV Performance Tests: https://boofcv.org/index.php?title=Performance:QrCode
- ZXing Test Images: https://github.com/zxing/zxing/tree/master/core/src/test/resources

Place downloaded images in this folder to include them in benchmarks.

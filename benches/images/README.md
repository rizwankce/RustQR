# Benchmark Test Images

Benchmark datasets live under this folder.

## Supported Formats
- PNG
- JPG/JPEG
- GIF
- BMP

## Image Requirements
- QR codes should be clearly visible
- Images can be any size (will be processed at original resolution)
- Multiple QR codes per image supported

## Layout
- `boofcv/` - BoofCV QR benchmark dataset (16 categories)
- `custom/` - your own images for quick experiments

## Benchmarking
Run benchmarks with:
`cargo bench --bench real_qr_images`

Optional environment variables:
- `QR_DATASET_ROOT` (default: `benches/images/boofcv`)
- `QR_BENCH_LIMIT` (default: `5`, set to `0` for no limit)
- `QR_SMOKE` (set to `1` to use `_smoke.txt` inside the dataset root)

## Sources
Test images can be downloaded from:
- Dynamsoft QR Benchmark: https://www.dynamsoft.com/codepool/qr-code-reading-benchmark-and-comparison.html
- BoofCV Performance Tests: https://boofcv.org/index.php?title=Performance:QrCode
- ZXing Test Images: https://github.com/zxing/zxing/tree/master/core/src/test/resources

Place downloaded images in this folder to include them in benchmarks.

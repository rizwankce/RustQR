# Benchmark Test Images

Place QR code images here for performance testing.

## Supported Formats
- PNG
- JPG/JPEG
- GIF
- BMP

## Image Requirements
- QR codes should be clearly visible
- Images can be any size (will be processed at original resolution)
- Multiple QR codes per image supported

## Benchmarking
Run benchmarks with: `cargo bench -- qr_images`

## Sources
Test images can be downloaded from:
- Dynamsoft QR Benchmark: https://www.dynamsoft.com/codepool/qr-code-reading-benchmark-and-comparison.html
- BoofCV Performance Tests: https://boofcv.org/index.php?title=Performance:QrCode
- ZXing Test Images: https://github.com/zxing/zxing/tree/master/core/src/test/resources

Place downloaded images in this folder to include them in benchmarks.

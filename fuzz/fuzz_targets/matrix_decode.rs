#![no_main]

use libfuzzer_sys::fuzz_target;
use rust_qr::decoder::qr_decoder::{MatrixErasureEvidence, QrDecoder};
use rust_qr::BitMatrix;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let (dimension, version) = dimensions(data[0]);
    let mut matrix = BitMatrix::new(dimension, dimension);
    for y in 0..dimension {
        for x in 0..dimension {
            let source = data[(x + y * dimension) % data.len()];
            matrix.set(x, y, source.rotate_left((x % 8) as u32) & 1 != 0);
        }
    }

    // This reaches format parsing, deinterleaving, Reed-Solomon correction,
    // and payload parsing whenever the arbitrary modules resemble a QR code.
    let _ = QrDecoder::decode_matrix(&matrix, version);

    // Exercise the separate confidence/erasure validation and correction path
    // without allowing a fuzzer input to allocate an unbounded sidecar.
    if dimension >= 21 && dimension == 17 + 4 * version as usize {
        let confidence: Vec<u8> = (0..dimension * dimension)
            .map(|index| data[(index + 1) % data.len()])
            .collect();
        let _ = QrDecoder::decode_matrix_with_erasures(
            &matrix,
            version,
            MatrixErasureEvidence::ModuleConfidence(&confidence),
        );
    }
});

fn dimensions(selector: u8) -> (usize, u8) {
    match selector % 10 {
        0 => (0, 1),
        1 => (20, 1),
        2 => (22, 1),
        3 => (21, 0),
        4 => (21, 41),
        5 => (21, 1),
        6 => (25, 2),
        7 => (45, 7),
        8 => (93, 19),
        _ => (177, 40),
    }
}

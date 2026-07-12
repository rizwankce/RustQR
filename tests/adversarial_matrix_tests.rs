use rust_qr::BitMatrix;
use rust_qr::decoder::qr_decoder::{MatrixDecodeError, QrDecoder};

fn golden_v1_matrix() -> BitMatrix {
    let text = include_str!("conformance/fixtures/golden-v1-m.matrix");
    let rows: Vec<_> = text.lines().filter(|line| !line.is_empty()).collect();
    let mut matrix = BitMatrix::new(rows[0].len(), rows.len());
    for (y, row) in rows.iter().enumerate() {
        for (x, module) in row.bytes().enumerate() {
            matrix.set(x, y, module == b'1');
        }
    }
    matrix
}

fn seeded_noise(dimension: usize, seed: u64) -> BitMatrix {
    let mut state = seed;
    let mut matrix = BitMatrix::new(dimension, dimension);
    for y in 0..dimension {
        for x in 0..dimension {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            matrix.set(x, y, state >> 63 != 0);
        }
    }
    matrix
}

#[test]
fn deterministic_qr_sized_noise_never_returns_a_payload() {
    for (version, seed) in [(1, 0x91_u64), (2, 0x2a), (7, 0x71), (40, 0x40)] {
        let dimension = 17 + 4 * version as usize;
        let result = QrDecoder::decode_matrix(&seeded_noise(dimension, seed), version);
        assert!(
            matches!(result, Err(MatrixDecodeError::DecodeFailed)),
            "version {version} seed {seed:#x} unexpectedly decoded: {result:?}"
        );
    }
}

#[test]
fn structural_damage_beyond_recovery_tolerance_is_rejected() {
    let mut finder_damage = golden_v1_matrix();
    for x in 0..4 {
        finder_damage.toggle(x, 0);
    }
    assert!(matches!(
        QrDecoder::decode_matrix(&finder_damage, 1),
        Err(MatrixDecodeError::DecodeFailed)
    ));

    let mut timing_damage = golden_v1_matrix();
    for x in 8..12 {
        timing_damage.toggle(x, 6);
    }
    assert!(matches!(
        QrDecoder::decode_matrix(&timing_damage, 1),
        Err(MatrixDecodeError::DecodeFailed)
    ));
}

#[test]
fn malformed_matrix_dimensions_return_structured_errors() {
    for (dimension, version, expected) in [
        (0, 1, MatrixDecodeError::InvalidDimensions),
        (20, 1, MatrixDecodeError::InvalidDimensions),
        (22, 1, MatrixDecodeError::VersionDimensionMismatch),
        (21, 0, MatrixDecodeError::InvalidDimensions),
        (21, 41, MatrixDecodeError::InvalidDimensions),
    ] {
        assert!(
            matches!(
                QrDecoder::decode_matrix(&BitMatrix::new(dimension, dimension), version),
                Err(error) if error == expected
            ),
            "dimension {dimension}, version {version}"
        );
    }
}

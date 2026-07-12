use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rust_qr::{BitMatrix, decoder::qr_decoder::QrDecoder};

/// The checked-in V1-M numeric fixture is a clean, correctly oriented Model 2
/// matrix. It therefore exercises the public matrix API's normal canonical
/// traversal rather than a photographic detector or damaged-symbol recovery
/// scenario.
const CANONICAL_V1_M_NUMERIC: &str =
    include_str!("../tests/conformance/fixtures/golden-v1-m.matrix");

fn canonical_v1_m_numeric_matrix() -> BitMatrix {
    let rows: Vec<&str> = CANONICAL_V1_M_NUMERIC.lines().collect();
    let width = rows.first().expect("fixture has a first row").len();
    let mut matrix = BitMatrix::new(width, rows.len());

    for (y, row) in rows.iter().enumerate() {
        assert_eq!(row.len(), width, "fixture is rectangular");
        for (x, module) in row.bytes().enumerate() {
            match module {
                b'0' => {}
                b'1' => matrix.set(x, y, true),
                _ => panic!("fixture contains only binary modules"),
            }
        }
    }

    matrix
}

fn bench_canonical_matrix_decode(c: &mut Criterion) {
    let matrix = canonical_v1_m_numeric_matrix();
    let decoded = QrDecoder::decode_matrix(&matrix, 1).expect("clean fixture decodes");
    assert_eq!(decoded.data, b"4376471154038");

    c.bench_function("decode_matrix/canonical_v1_m_numeric", |b| {
        b.iter(|| {
            QrDecoder::decode_matrix(black_box(&matrix), black_box(1))
                .expect("clean fixture decodes")
        })
    });
}

criterion_group!(benches, bench_canonical_matrix_decode);
criterion_main!(benches);

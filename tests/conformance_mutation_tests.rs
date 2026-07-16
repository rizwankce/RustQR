use rust_qr::BitMatrix;
use rust_qr::decoder::qr_decoder::{
    MatrixDataMode, MatrixDecodeError, MatrixErasureEvidence, QrDecoder,
};
use rust_qr::matrix_core;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("conformance")
}

fn mode(value: &str) -> MatrixDataMode {
    match value {
        "numeric" => MatrixDataMode::Numeric,
        "alphanumeric" => MatrixDataMode::Alphanumeric,
        "byte" => MatrixDataMode::Byte,
        "kanji" => MatrixDataMode::Kanji,
        "eci" => MatrixDataMode::Eci,
        "gs1_fnc1" => MatrixDataMode::Gs1Fnc1,
        "structured_append" => MatrixDataMode::StructuredAppend,
        other => panic!("mutation parent has unsupported mode {other}"),
    }
}

fn decode_hex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            u8::from_str_radix(std::str::from_utf8(pair).expect("ASCII hex"), 16)
                .expect("valid hex")
        })
        .collect()
}

fn matrix(metadata: &Value) -> BitMatrix {
    let image = image::open(root().join(metadata["path"].as_str().expect("matrix path")))
        .expect("read mutation image")
        .to_luma8();
    let dimension = metadata["symbol_dimension"].as_u64().expect("dimension") as usize;
    let quiet = metadata["quiet_zone_modules"].as_u64().expect("quiet zone") as usize;
    let scale = metadata["pixels_per_module"].as_u64().expect("scale") as usize;
    let mut result = BitMatrix::new(dimension, dimension);
    for y in 0..dimension {
        for x in 0..dimension {
            result.set(
                x,
                y,
                image
                    .get_pixel(
                        ((quiet + x) * scale + scale / 2) as u32,
                        ((quiet + y) * scale + scale / 2) as u32,
                    )
                    .0[0]
                    < 128,
            );
        }
    }
    result
}

fn core_matrix(matrix: &BitMatrix) -> matrix_core::BitMatrix {
    let mut result = matrix_core::BitMatrix::new(matrix.width(), matrix.height());
    for y in 0..matrix.height() {
        for x in 0..matrix.width() {
            result.set(x, y, matrix.get(x, y));
        }
    }
    result
}

#[test]
fn materialized_mapping_backed_mutations_have_expected_results() {
    let corpus: Value = serde_json::from_str(
        &fs::read_to_string(root().join("manifest.json")).expect("read source manifest"),
    )
    .expect("parse source manifest");
    let mutations: Value = serde_json::from_str(
        &fs::read_to_string(root().join("mutations.json")).expect("read mutation manifest"),
    )
    .expect("parse mutation manifest");
    assert_eq!(
        mutations["schema_version"],
        "rustqr.conformance-mutations.v1"
    );

    let cases = corpus["cases"].as_array().expect("source cases");
    let mut executed = 0usize;
    for mutation in mutations["mutations"].as_array().expect("mutations") {
        if mutation["status"] != "materialized" || mutation["kind"] == "correctable_erasures" {
            continue;
        }
        let parent_id = mutation["parent_id"].as_str().expect("parent id");
        let parent = cases
            .iter()
            .find(|case| case["id"] == parent_id)
            .expect("mutation parent");
        let result = QrDecoder::decode_matrix_for_mode(
            &matrix(&mutation["matrix"]),
            parent["version"].as_u64().expect("version") as u8,
            mode(parent["mode"].as_str().expect("mode")),
        );
        executed += 1;
        match mutation["expected_outcome"].as_str().expect("outcome") {
            "decode_success" => assert_eq!(
                result.expect("correctable mutation should decode").data,
                decode_hex(
                    mutation["expected_raw_payload_hex"]
                        .as_str()
                        .expect("payload"),
                ),
                "{}",
                mutation["id"],
            ),
            "reject" => assert!(result.is_err(), "{} was accepted", mutation["id"]),
            other => panic!("unknown expected outcome {other}"),
        }
    }
    assert!(executed > 0);
}

#[test]
fn block_layout_mutations_are_never_ambiguous() {
    let mutations: Value = serde_json::from_str(
        &fs::read_to_string(root().join("mutations.json")).expect("read mutation manifest"),
    )
    .expect("parse mutation manifest");
    let layouts: Vec<&Value> = mutations["mutations"]
        .as_array()
        .expect("mutations")
        .iter()
        .filter(|mutation| mutation["kind"] == "invalid_block_layout")
        .collect();
    assert!(!layouts.is_empty());
    for mutation in layouts {
        assert_ne!(mutation["status"], "planned", "{}", mutation["id"]);
        if mutation["status"] == "materialized" {
            assert_eq!(mutation["expected_outcome"], "reject");
            assert!(
                mutation["errors_per_affected_block"].as_u64()
                    > mutation["correction_limit_per_block"].as_u64(),
                "{} must exceed the per-block correction limit",
                mutation["id"]
            );
            assert_eq!(
                mutation["affected_blocks"]
                    .as_array()
                    .expect("affected blocks")
                    .len(),
                2
            );
        } else {
            assert_eq!(mutation["status"], "not_applicable");
        }
    }
}

#[test]
fn materialized_erasure_sidecars_decode_at_the_supported_boundary() {
    let corpus: Value = serde_json::from_str(
        &fs::read_to_string(root().join("manifest.json")).expect("read source manifest"),
    )
    .expect("parse source manifest");
    let mutations: Value = serde_json::from_str(
        &fs::read_to_string(root().join("mutations.json")).expect("read mutation manifest"),
    )
    .expect("parse mutation manifest");
    let cases = corpus["cases"].as_array().expect("source cases");
    let mut executed = 0usize;
    for mutation in mutations["mutations"].as_array().expect("mutations") {
        if mutation["kind"] != "correctable_erasures" {
            continue;
        }
        assert_eq!(mutation["status"], "materialized");
        let sidecar = root().join(
            mutation["confidence_sidecar"]["path"]
                .as_str()
                .expect("sidecar path"),
        );
        assert!(sidecar.is_file(), "{}", sidecar.display());
        let evidence: Value =
            serde_json::from_str(&fs::read_to_string(sidecar).expect("read confidence sidecar"))
                .expect("parse confidence sidecar");
        let coordinates: Vec<(usize, usize)> = evidence["module_coordinates"]
            .as_array()
            .expect("module coordinates")
            .iter()
            .map(|coordinate| {
                let pair = coordinate.as_array().expect("coordinate pair");
                (
                    pair[0].as_u64().expect("x") as usize,
                    pair[1].as_u64().expect("y") as usize,
                )
            })
            .collect();
        let parent = cases
            .iter()
            .find(|case| case["id"] == mutation["parent_id"])
            .expect("mutation parent");
        let qr_matrix = matrix(&mutation["matrix"]);
        let result = QrDecoder::decode_matrix_with_erasures(
            &qr_matrix,
            parent["version"].as_u64().expect("version") as u8,
            MatrixErasureEvidence::ErasedModules(&coordinates),
        );
        assert_eq!(
            result.expect("known erasures should decode").data,
            decode_hex(
                mutation["expected_raw_payload_hex"]
                    .as_str()
                    .expect("payload"),
            ),
            "{}",
            mutation["id"],
        );
        let strict_core = matrix_core::decode_with_erasures(
            &core_matrix(&qr_matrix),
            parent["version"].as_u64().expect("version") as u8,
            matrix_core::MatrixErasureEvidence::ErasedModules(&coordinates),
        )
        .expect("strict core known erasures should decode");
        assert_eq!(
            strict_core.data,
            decode_hex(
                mutation["expected_raw_payload_hex"]
                    .as_str()
                    .expect("payload"),
            ),
            "{}: strict core erasure parity",
            mutation["id"],
        );
        executed += 1;
    }
    assert_eq!(executed, 34);
}

#[test]
fn matrix_erasure_evidence_is_validated() {
    let matrix = BitMatrix::new(21, 21);
    assert!(matches!(
        QrDecoder::decode_matrix_with_erasures(
            &matrix,
            1,
            MatrixErasureEvidence::ModuleConfidence(&[255; 8]),
        ),
        Err(MatrixDecodeError::InvalidConfidenceLength)
    ));
    assert!(matches!(
        QrDecoder::decode_matrix_with_erasures(
            &matrix,
            1,
            MatrixErasureEvidence::ErasedModules(&[(21, 0)]),
        ),
        Err(MatrixDecodeError::ErasureModuleOutOfBounds)
    ));
}

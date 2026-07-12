use rust_qr::BitMatrix;
use rust_qr::decoder::qr_decoder::{MatrixDataMode, MatrixDecodeError, QrDecoder};
use rust_qr::decoder::version::VersionInfo;
use rust_qr::{ECLevel, Version};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/conformance")
}

fn parse_matrix(path: &Path) -> BitMatrix {
    let text = fs::read_to_string(path).expect("read matrix fixture");
    let rows: Vec<_> = text.lines().filter(|line| !line.is_empty()).collect();
    assert!(!rows.is_empty(), "empty matrix fixture");
    let width = rows[0].len();
    assert!(rows.iter().all(|row| row.len() == width));
    let mut matrix = BitMatrix::new(width, rows.len());
    for (y, row) in rows.iter().enumerate() {
        for (x, value) in row.bytes().enumerate() {
            match value {
                b'0' => {}
                b'1' => matrix.set(x, y, true),
                _ => panic!("matrix fixture contains a non-binary value"),
            }
        }
    }
    matrix
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
        other => panic!("unknown fixture mode {other}"),
    }
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).expect("ASCII hex");
            u8::from_str_radix(text, 16).expect("valid fixture hex")
        })
        .collect()
}

fn ec_level(value: &str) -> ECLevel {
    match value {
        "L" => ECLevel::L,
        "M" => ECLevel::M,
        "Q" => ECLevel::Q,
        "H" => ECLevel::H,
        other => panic!("unknown EC level {other}"),
    }
}

fn load_generated_matrix(path: &Path, dimension: usize, quiet: usize, scale: usize) -> BitMatrix {
    let image = image::open(path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
        .to_luma8();
    let expected_pixels = (dimension + quiet * 2) * scale;
    assert_eq!(
        image.width() as usize,
        expected_pixels,
        "{}",
        path.display()
    );
    assert_eq!(
        image.height() as usize,
        expected_pixels,
        "{}",
        path.display()
    );
    let mut matrix = BitMatrix::new(dimension, dimension);
    for y in 0..dimension {
        for x in 0..dimension {
            let pixel_x = (quiet + x) * scale + scale / 2;
            let pixel_y = (quiet + y) * scale + scale / 2;
            matrix.set(
                x,
                y,
                image.get_pixel(pixel_x as u32, pixel_y as u32).0[0] < 128,
            );
        }
    }
    matrix
}

fn assert_generated_supported_corpus(root: &Path, expected_cases: Option<usize>) {
    let text = fs::read_to_string(root.join("manifest.json")).expect("read corpus manifest");
    let manifest: Value = serde_json::from_str(&text).expect("parse corpus manifest");
    let mut failures = Vec::new();
    let mut executed = 0usize;
    for fixture in manifest["cases"].as_array().expect("corpus cases") {
        if fixture["matrix"]["status"] != "generated" {
            continue;
        }
        executed += 1;
        let id = fixture["id"].as_str().expect("case id");
        let version = fixture["version"].as_u64().expect("version") as u8;
        let dimension = fixture["matrix"]["symbol_dimension"]
            .as_u64()
            .expect("symbol dimension") as usize;
        let quiet = fixture["matrix"]["quiet_zone_modules"]
            .as_u64()
            .expect("quiet zone") as usize;
        let scale = fixture["matrix"]["pixels_per_module"]
            .as_u64()
            .expect("pixel scale") as usize;
        let matrix_path = root.join(fixture["matrix"]["path"].as_str().expect("matrix path"));
        assert_eq!(
            fixture["matrix"]["sha256"].as_str().map(str::len),
            Some(64),
            "{id}: matrix checksum is missing"
        );
        let matrix = load_generated_matrix(&matrix_path, dimension, quiet, scale);
        let expected = decode_hex(fixture["payload_hex"].as_str().expect("payload"));
        match QrDecoder::decode_matrix_for_mode(
            &matrix,
            version,
            mode(fixture["mode"].as_str().expect("mode")),
        ) {
            Ok(decoded)
                if decoded.data == expected
                    && decoded.version == Version::Model2(version)
                    && decoded.error_correction
                        == ec_level(
                            fixture["expected"]["ec_level"].as_str().expect("EC level"),
                        )
                    && decoded.mask_pattern as u64
                        == fixture["expected"]["mask"].as_u64().expect("mask") => {}
            Ok(decoded) => failures.push(format!(
                "{id}: payload {:?}, version {:?}, EC {:?}, mask {:?}",
                decoded.data, decoded.version, decoded.error_correction, decoded.mask_pattern
            )),
            Err(error) => failures.push(format!("{id}: {error:?}")),
        }
    }
    assert_eq!(
        expected_cases.unwrap_or(executed),
        executed,
        "generated case count"
    );
    assert!(
        executed > 0,
        "no generated supported fixtures were executed"
    );
    assert!(
        failures.is_empty(),
        "{}/{} generated fixtures failed:\n{}",
        failures.len(),
        executed,
        failures.join("\n")
    );
}

struct TemporaryCorpus {
    path: PathBuf,
}

impl TemporaryCorpus {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rustqr-full-conformance-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create temporary corpus directory");
        Self { path }
    }
}

impl Drop for TemporaryCorpus {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run_python(script: &str, arguments: &[&str]) {
    let status = Command::new("python3")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join(script))
        .args(arguments)
        .status()
        .unwrap_or_else(|error| panic!("run {script}: {error}"));
    assert!(status.success(), "{script} failed with {status}");
}

#[test]
fn manifest_executes_valid_invalid_and_unsupported_fixtures() {
    let root = fixture_root();
    let manifest_text = fs::read_to_string(root.join("cases.json")).expect("read manifest");
    let manifest: Value = serde_json::from_str(&manifest_text).expect("parse manifest");
    assert_eq!(manifest["schema_version"], "rustqr.matrix-fixtures.v1");

    for fixture in manifest["cases"].as_array().expect("fixture cases") {
        let id = fixture["id"].as_str().expect("case id");
        let version = fixture["version"].as_u64().expect("version") as u8;
        let fixture_mode = mode(fixture["mode"].as_str().expect("mode"));
        let matrix = fixture["matrix"]
            .as_str()
            .map(|path| parse_matrix(&root.join(path)))
            .unwrap_or_else(|| BitMatrix::new(21, 21));
        let result = QrDecoder::decode_matrix_for_mode(&matrix, version, fixture_mode);

        match fixture["expectation"].as_str().expect("expectation") {
            "valid" => {
                let decoded = result.unwrap_or_else(|error| panic!("{id}: {error:?}"));
                let expected = decode_hex(fixture["payload_hex"].as_str().expect("payload"));
                assert_eq!(decoded.data, expected, "{id}");
            }
            "invalid" => assert!(
                matches!(result, Err(MatrixDecodeError::DecodeFailed)),
                "{id}"
            ),
            "unsupported" => assert!(
                matches!(result, Err(MatrixDecodeError::UnsupportedMode(_))),
                "{id}"
            ),
            expectation => panic!("{id}: unknown expectation {expectation}"),
        }
    }
}

#[test]
fn fixture_manifest_and_seeded_boundaries_are_reproducible() {
    let text = fs::read_to_string(fixture_root().join("cases.json")).expect("read manifest");
    let first: Value = serde_json::from_str(&text).expect("parse manifest");
    let second: Value = serde_json::from_str(&text).expect("parse manifest again");
    assert_eq!(first, second);

    fn positions(mut state: u64, count: usize, limit: usize) -> Vec<usize> {
        let mut result = Vec::new();
        while result.len() < count {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let position = state as usize % limit;
            if !result.contains(&position) {
                result.push(position);
            }
        }
        result
    }
    let seed = first["seed"].as_u64().expect("manifest seed");
    assert_eq!(positions(seed, 10, 26), positions(seed, 10, 26));
}

#[test]
fn structural_invalid_fixtures_are_materialized_and_rejected() {
    let root = fixture_root();
    let text = fs::read_to_string(root.join("structural-invalid.json"))
        .expect("read structural-invalid manifest");
    let manifest: Value = serde_json::from_str(&text).expect("parse structural-invalid manifest");
    assert_eq!(
        manifest["schema_version"],
        "rustqr.structural-invalid-fixtures.v1"
    );

    let cases = manifest["cases"].as_array().expect("structural cases");
    assert_eq!(
        cases.len(),
        4,
        "format, version, remainder, and padding fixtures are required"
    );
    for fixture in cases {
        let id = fixture["id"].as_str().expect("case id");
        let matrix = parse_matrix(
            &root
                .join("fixtures")
                .join(fixture["matrix"].as_str().expect("matrix path")),
        );
        let version = match fixture["kind"].as_str().expect("mutation kind") {
            "invalid_format" => 1,
            "invalid_version" => 7,
            "invalid_remainder" => 2,
            "invalid_padding" => 1,
            kind => panic!("{id}: unsupported structural mutation {kind}"),
        };
        assert!(
            matches!(
                QrDecoder::decode_matrix_for_mode(&matrix, version, MatrixDataMode::Byte),
                Err(MatrixDecodeError::DecodeFailed)
            ),
            "{id} was accepted"
        );
        assert_eq!(
            fixture["sha256"].as_str().expect("fixture checksum").len(),
            64,
            "{id}: checksum must be a SHA-256 hex digest"
        );
    }
    assert_eq!(
        manifest["provenance"]["placement_map"],
        "scripts/qr_spec_mapping.py"
    );
    assert_eq!(
        manifest["provenance"]["padding_backend"],
        "python-qrcode==8.2"
    );
}

#[test]
fn remainder_and_padding_mutations_have_provable_coordinates() {
    let root = fixture_root();
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(root.join("structural-invalid.json")).expect("read manifest"),
    )
    .expect("parse manifest");
    let cases = manifest["cases"].as_array().expect("structural cases");

    let remainder = cases
        .iter()
        .find(|case| case["kind"] == "invalid_remainder")
        .expect("remainder fixture");
    assert_eq!(remainder["mutation"]["remainder_module_count"], 7);
    assert_eq!(
        remainder["mutation"]["coordinate"],
        serde_json::json!([0, 13])
    );
    assert_eq!(remainder["mutation"]["unmasked_after"], 1);

    let padding = cases
        .iter()
        .find(|case| case["kind"] == "invalid_padding")
        .expect("padding fixture");
    assert_eq!(padding["mutation"]["data_codeword_index"], 8);
    assert_eq!(padding["mutation"]["expected_pad_codeword"], "ec");
    assert_eq!(padding["mutation"]["replacement_codeword"], "ed");
    assert_eq!(
        padding["mutation"]["module_coordinates_msb_first"]
            .as_array()
            .expect("module coordinates")
            .len(),
        8
    );
    assert_eq!(
        padding["mutation"]["reed_solomon_parity"],
        "regenerated with python-qrcode==8.2"
    );
}

#[test]
fn version_fixture_mutation_replaces_valid_redundant_version_information() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("conformance");
    let valid = load_generated_matrix(&corpus.join("matrices/v07-L-m0-byte.png"), 45, 4, 4);
    let invalid = parse_matrix(&fixture_root().join("fixtures/invalid-version-v7.matrix"));

    assert_eq!(VersionInfo::extract(&valid), Some(7));
    assert_eq!(VersionInfo::extract(&invalid), None);
}

#[test]
fn generated_supported_corpus_reaches_one_hundred_percent() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("conformance");
    assert_generated_supported_corpus(&root, Some(30));
}

#[test]
#[ignore = "regenerates and decodes the 3,840-case Model 2 grid"]
fn generated_full_supported_corpus_reaches_one_hundred_percent() {
    let corpus = TemporaryCorpus::new();
    let manifest = corpus.path.join("manifest.json");
    let manifest_string = manifest.to_str().expect("UTF-8 temporary path");
    run_python(
        "scripts/generate_conformance_manifest.py",
        &["--output", manifest_string, "--full"],
    );
    run_python(
        "scripts/materialize_conformance_matrices.py",
        &["--manifest", manifest_string],
    );
    assert_generated_supported_corpus(&corpus.path, Some(40 * 4 * 8 * 3));
}

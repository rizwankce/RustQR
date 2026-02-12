#![forbid(unsafe_code)]

pub mod config;
pub mod pipeline;
pub mod telemetry;
pub mod types;

pub use config::DetectConfig;
pub use telemetry::DetectionRunReport;
pub use types::QrCode;

/// Detect QR codes from an RGB image buffer.
pub fn detect(image: &[u8], width: usize, height: usize) -> Vec<QrCode> {
    let config = DetectConfig::default();
    pipeline::detect_with_config(image, width, height, &config).codes
}

/// Detect QR codes and return stage telemetry for benchmark/debug pipelines.
pub fn detect_with_report(image: &[u8], width: usize, height: usize) -> DetectionRunReport {
    let config = DetectConfig::default();
    pipeline::detect_with_config(image, width, height, &config)
}

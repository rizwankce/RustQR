use std::sync::OnceLock;

fn parse_env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

fn parse_env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(default)
}

fn parse_env_u8(name: &str, default: u8) -> u8 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u8>().ok())
        .unwrap_or(default)
}

fn parse_env_bool_u8(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u8>().ok())
        .map(|v| v != 0)
        .unwrap_or(default)
}

static CANDIDATE_TIME_BUDGET_MS: OnceLock<u64> = OnceLock::new();

pub(crate) fn candidate_time_budget_ms() -> u64 {
    *CANDIDATE_TIME_BUDGET_MS.get_or_init(|| parse_env_u64("QR_CANDIDATE_TIME_BUDGET_MS", 120))
}

static FORMAT_FALLBACK_FULL_EC: OnceLock<bool> = OnceLock::new();

pub(crate) fn format_fallback_full_ec() -> bool {
    *FORMAT_FALLBACK_FULL_EC.get_or_init(|| parse_env_bool_u8("QR_FORMAT_FALLBACK_FULL_EC", true))
}

static STRICT_FALLBACK_VERSION_MATCH: OnceLock<bool> = OnceLock::new();

pub(crate) fn strict_fallback_version_match() -> bool {
    *STRICT_FALLBACK_VERSION_MATCH
        .get_or_init(|| parse_env_bool_u8("QR_STRICT_FALLBACK_VERSION_MATCH", false))
}

static RELAXED_FINDER_MISMATCH: OnceLock<usize> = OnceLock::new();

pub(crate) fn relaxed_finder_mismatch() -> usize {
    *RELAXED_FINDER_MISMATCH
        .get_or_init(|| parse_env_usize("QR_RELAXED_FINDER_MISMATCH", 10).clamp(4, 16))
}

static BEAM_TOP_N: OnceLock<usize> = OnceLock::new();

pub(crate) fn beam_top_n() -> usize {
    *BEAM_TOP_N.get_or_init(|| parse_env_usize("QR_BEAM_TOP_N", 6).clamp(2, 12))
}

static BEAM_MAX_ATTEMPTS: OnceLock<usize> = OnceLock::new();

pub(crate) fn beam_max_attempts() -> usize {
    *BEAM_MAX_ATTEMPTS.get_or_init(|| parse_env_usize("QR_BEAM_MAX_ATTEMPTS", 12).clamp(1, 64))
}

static BEAM_MAX_DEPTH: OnceLock<usize> = OnceLock::new();

pub(crate) fn beam_max_depth() -> usize {
    *BEAM_MAX_DEPTH.get_or_init(|| parse_env_usize("QR_BEAM_MAX_DEPTH", 2).clamp(1, 3))
}

static BEAM_CONF_THRESHOLD: OnceLock<u8> = OnceLock::new();

pub(crate) fn beam_conf_threshold() -> u8 {
    *BEAM_CONF_THRESHOLD.get_or_init(|| parse_env_u8("QR_BEAM_CONF_THRESHOLD", 36))
}

static RS_ERASURE_CONF_THRESHOLD: OnceLock<u8> = OnceLock::new();

pub(crate) fn rs_erasure_conf_threshold() -> u8 {
    *RS_ERASURE_CONF_THRESHOLD.get_or_init(|| parse_env_u8("QR_RS_ERASURE_CONF_THRESHOLD", 40))
}

static RS_MAX_ERASURES: OnceLock<Option<usize>> = OnceLock::new();

pub(crate) fn rs_max_erasures_override() -> Option<usize> {
    *RS_MAX_ERASURES.get_or_init(|| {
        std::env::var("QR_RS_MAX_ERASURES")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
    })
}

static IMAGE_DECODE_ATTEMPT_BUDGET: OnceLock<usize> = OnceLock::new();

pub(crate) fn image_decode_attempt_budget() -> usize {
    *IMAGE_DECODE_ATTEMPT_BUDGET
        .get_or_init(|| parse_env_usize("QR_MAX_IMAGE_DECODE_ATTEMPTS", 72).max(1))
}

static BLUR_DISABLE_RECOVERY_THRESHOLD: OnceLock<f32> = OnceLock::new();

/// Blur metric threshold below which expensive recovery (RS erasure, subpixel) is disabled.
/// Lower values = more blur. Default: 8.0 (images blurrier than this skip expensive recovery)
pub(crate) fn blur_disable_recovery_threshold() -> f32 {
    *BLUR_DISABLE_RECOVERY_THRESHOLD
        .get_or_init(|| parse_env_f32("QR_BLUR_DISABLE_RECOVERY_THRESHOLD", 8.0).max(0.0))
}

static RS_ERASURE_GLOBAL_CAP: OnceLock<usize> = OnceLock::new();

/// Hard cap on total RS erasure attempts per image. Default: 100 (0 = unlimited)
pub(crate) fn rs_erasure_global_cap() -> usize {
    *RS_ERASURE_GLOBAL_CAP.get_or_init(|| parse_env_usize("QR_RS_ERASURE_GLOBAL_CAP", 100))
}

fn parse_env_f32(name: &str, default: f32) -> f32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<f32>().ok())
        .unwrap_or(default)
}

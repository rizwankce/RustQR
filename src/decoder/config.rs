//! Library defaults for recovery guardrails.
//!
//! These values deliberately do not read process environment variables.  An
//! embedding application must select request behaviour through the public
//! `DecoderOptions` API instead of inheriting ambient process state.

pub(crate) const fn candidate_time_budget_ms() -> u64 {
    300
}
pub(crate) const fn strict_fallback_version_match() -> bool {
    false
}
pub(crate) const fn relaxed_finder_mismatch() -> usize {
    10
}
pub(crate) const fn beam_top_n() -> usize {
    6
}
pub(crate) const fn beam_max_attempts() -> usize {
    12
}
pub(crate) const fn beam_max_depth() -> usize {
    2
}
pub(crate) const fn beam_conf_threshold() -> u8 {
    36
}
pub(crate) const fn beam_time_budget_ms() -> u64 {
    50
}
pub(crate) const fn beam_uncertain_max() -> usize {
    80
}
pub(crate) const fn max_groups_to_rank() -> usize {
    16
}
pub(crate) const fn rs_erasure_conf_threshold() -> u8 {
    40
}
pub(crate) const fn rs_max_erasures_override() -> Option<usize> {
    None
}
pub(crate) const fn image_decode_attempt_budget() -> usize {
    128
}
pub(crate) const fn blur_disable_recovery_threshold() -> f32 {
    8.0
}
pub(crate) const fn global_time_budget_ms() -> u64 {
    2_000
}

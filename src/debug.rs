use std::sync::OnceLock;

static DEBUG_ENABLED: OnceLock<bool> = OnceLock::new();

pub(crate) fn debug_enabled() -> bool {
    *DEBUG_ENABLED.get_or_init(|| std::env::var("QR_DEBUG").is_ok())
}

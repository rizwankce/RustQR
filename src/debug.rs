pub(crate) fn debug_enabled() -> bool {
    std::env::var("QR_DEBUG").is_ok()
}

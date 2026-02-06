use crate::decoder::format::FormatInfo;
use crate::decoder::qr_decoder::{orientation, payload};
use crate::models::{BitMatrix, ECLevel, MaskPattern, QRCode};

pub(super) fn decode_from_matrix(qr_matrix: &BitMatrix, version_num: u8) -> Option<QRCode> {
    let orientations = orientation::candidate_orientations(qr_matrix);
    if orientations.is_empty() {
        return None;
    }

    let traversal_opts = [(true, false), (true, true), (false, false), (false, true)];

    // Fast path: if format BCH extraction succeeds, use only that format.
    for oriented in &orientations {
        if !orientation::version_matches_candidate(oriented, version_num) {
            continue;
        }
        if let Some(format_info) = FormatInfo::extract(oriented) {
            for &(start_upward, swap_columns) in &traversal_opts {
                if let Some(qr) = payload::try_decode_single(
                    oriented,
                    version_num,
                    &format_info,
                    start_upward,
                    swap_columns,
                    true,
                    false,
                ) {
                    return Some(qr);
                }
            }
        }
    }

    // Last-resort fallback: limited EC/mask subset (not full 32-combo brute force).
    for oriented in &orientations {
        if !orientation::version_matches_candidate(oriented, version_num) {
            continue;
        }
        let fallback_ec = [ECLevel::L, ECLevel::M];
        for &ec in &fallback_ec {
            for mask in 0..8u8 {
                if let Some(mask_pattern) = MaskPattern::from_bits(mask) {
                    let info = FormatInfo {
                        ec_level: ec,
                        mask_pattern,
                    };
                    for &(start_upward, swap_columns) in &traversal_opts {
                        if let Some(qr) = payload::try_decode_single(
                            oriented,
                            version_num,
                            &info,
                            start_upward,
                            swap_columns,
                            true,
                            false,
                        ) {
                            return Some(qr);
                        }
                    }
                }
            }
        }
    }

    None
}

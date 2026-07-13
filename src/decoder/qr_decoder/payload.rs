use crate::decoder::bitstream::BitstreamExtractor;
use crate::decoder::format::FormatInfo;
use crate::decoder::function_mask::FunctionMask;
use crate::decoder::modes::{
    alphanumeric::AlphanumericDecoder, kanji::KanjiDecoder, numeric::NumericDecoder,
};
use crate::decoder::qr_decoder::DecodeRequestContext;
use crate::decoder::reed_solomon::ReedSolomonDecoder;
use crate::decoder::tables::ec_block_info;
use crate::decoder::unmask::unmask;
use crate::decoder::version::VersionInfo;
use crate::models::{
    BitMatrix, ECLevel, Fnc1Position, QRCode, QRCodeMetadata, StructuredAppendInfo, Version,
};
fn record_erasure_hist(context: &mut DecodeRequestContext, count: usize) {
    let counters = context.counters_mut();
    match count {
        0 => {}
        1 => counters.rs_erasure_count_hist[0] += 1,
        2..=3 => counters.rs_erasure_count_hist[1] += 1,
        4..=6 => counters.rs_erasure_count_hist[2] += 1,
        _ => counters.rs_erasure_count_hist[3] += 1,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn try_decode_single(
    oriented: &BitMatrix,
    version_num: u8,
    format_info: &FormatInfo,
    start_upward: bool,
    swap_columns: bool,
    use_msb: bool,
    reverse_stream: bool,
    module_confidence: Option<&[u8]>,
    context: &mut DecodeRequestContext,
) -> Option<QRCode> {
    try_decode_single_internal(
        oriented,
        version_num,
        format_info,
        start_upward,
        swap_columns,
        use_msb,
        reverse_stream,
        module_confidence,
        false,
        context,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn try_decode_single_deterministic_erasures(
    oriented: &BitMatrix,
    version_num: u8,
    format_info: &FormatInfo,
    module_confidence: &[u8],
) -> Option<QRCode> {
    let mut context = DecodeRequestContext::default();
    [(true, false), (true, true), (false, false), (false, true)]
        .into_iter()
        .find_map(|(start_upward, swap_columns)| {
            try_decode_single_internal(
                oriented,
                version_num,
                format_info,
                start_upward,
                swap_columns,
                true,
                false,
                Some(module_confidence),
                true,
                &mut context,
            )
        })
}

#[allow(clippy::too_many_arguments)]
fn try_decode_single_internal(
    oriented: &BitMatrix,
    version_num: u8,
    format_info: &FormatInfo,
    start_upward: bool,
    swap_columns: bool,
    use_msb: bool,
    reverse_stream: bool,
    module_confidence: Option<&[u8]>,
    deterministic_erasures: bool,
    context: &mut DecodeRequestContext,
) -> Option<QRCode> {
    let dimension = oriented.width();
    let func = FunctionMask::new(version_num);
    let mut unmasked = oriented.clone();
    unmask(&mut unmasked, &format_info.mask_pattern, &func);

    let (bits, bit_confidence) = if let Some(conf) = module_confidence {
        BitstreamExtractor::extract_with_confidence(
            &unmasked,
            dimension,
            &func,
            start_upward,
            swap_columns,
            conf,
        )
    } else {
        (
            BitstreamExtractor::extract_with_options(
                &unmasked,
                dimension,
                &func,
                start_upward,
                swap_columns,
            ),
            Vec::new(),
        )
    };
    // ISO/IEC 18004 requires every module after the final complete raw
    // codeword to be zero before masking. Check the canonical traversal even
    // when a recovery traversal is being attempted, otherwise those modules
    // would be silently discarded by `bits_to_codewords_with_confidence`.
    let canonical_bits = BitstreamExtractor::extract(&unmasked, dimension, &func);
    if has_nonzero_remainder_bits(&canonical_bits) {
        context.counters_mut().nonzero_remainder_bit_rejections += 1;
        return None;
    }
    let (bits, bit_confidence) = if reverse_stream {
        let mut rev_bits = bits;
        rev_bits.reverse();
        let mut rev_conf = bit_confidence;
        rev_conf.reverse();
        (rev_bits, rev_conf)
    } else {
        (bits, bit_confidence)
    };

    let (codewords, codeword_confidence) = if use_msb {
        bits_to_codewords_with_confidence(&bits, &bit_confidence, true)
    } else {
        bits_to_codewords_with_confidence(&bits, &bit_confidence, false)
    };

    context.counters_mut().rs_candidate_attempts += 1;
    let data_codewords = deinterleave_and_correct_with_confidence(
        &codewords,
        version_num,
        format_info.ec_level,
        if codeword_confidence.is_empty() {
            None
        } else {
            Some(&codeword_confidence)
        },
        deterministic_erasures,
        context,
    )?;

    let decoded = match decode_payload_strict_result(&data_codewords, version_num) {
        Ok(decoded) => decoded,
        Err(PayloadDecodeError::UnsupportedMode(_)) => {
            context.counters_mut().unsupported_payloads += 1;
            return None;
        }
        Err(PayloadDecodeError::Malformed) => return None,
    };
    if decoded.data.is_empty() {
        return None;
    }

    let version = if dimension >= 45 {
        VersionInfo::extract(oriented)
            .map(Version::Model2)
            .unwrap_or(Version::Model2(version_num))
    } else {
        Version::Model2(version_num)
    };

    let mut qr = QRCode::new(
        decoded.data,
        decoded.content,
        version,
        format_info.ec_level,
        format_info.mask_pattern,
    );
    qr.metadata = decoded.metadata;
    Some(qr)
}

/// QR remainder bits are not payload bits and must be zero before masking.
///
/// The bitstream extractor yields all non-function modules, while the final
/// partial byte is specified as zero-valued remainder bits. Keeping this check
/// separate makes the post-format, pre-RS rejection observable without
/// changing any decode behaviour.
fn has_nonzero_remainder_bits(bits: &[bool]) -> bool {
    bits[bits.len() / 8 * 8..].iter().any(|bit| *bit)
}

#[allow(dead_code)]
pub(super) fn bits_to_codewords(bits: &[bool]) -> Vec<u8> {
    let mut codewords = Vec::with_capacity(bits.len() / 8);
    let mut idx = 0;
    while idx + 8 <= bits.len() {
        let mut byte = 0u8;
        for _ in 0..8 {
            byte = (byte << 1) | (bits[idx] as u8);
            idx += 1;
        }
        codewords.push(byte);
    }
    codewords
}

#[allow(dead_code)]
pub(super) fn bits_to_codewords_lsb(bits: &[bool]) -> Vec<u8> {
    let mut codewords = Vec::with_capacity(bits.len() / 8);
    let mut idx = 0;
    while idx + 8 <= bits.len() {
        let mut byte = 0u8;
        for bit in 0..8 {
            if bits[idx] {
                byte |= 1 << bit;
            }
            idx += 1;
        }
        codewords.push(byte);
    }
    codewords
}

#[allow(dead_code)]
pub(super) fn deinterleave_and_correct(
    codewords: &[u8],
    version: u8,
    ec_level: ECLevel,
) -> Option<Vec<u8>> {
    deinterleave_and_correct_with_confidence(
        codewords,
        version,
        ec_level,
        None,
        false,
        &mut DecodeRequestContext::default(),
    )
}

pub(super) fn deinterleave_and_correct_with_confidence(
    codewords: &[u8],
    version: u8,
    ec_level: ECLevel,
    codeword_confidence: Option<&[u8]>,
    deterministic_erasures: bool,
    context: &mut DecodeRequestContext,
) -> Option<Vec<u8>> {
    let info = ec_block_info(version, ec_level)?;
    let total = codewords.len();
    let ecc_total = info.num_blocks * info.ecc_per_block;
    if total < ecc_total {
        return None;
    }
    let data_total = total - ecc_total;
    if data_total == 0 {
        return None;
    }

    let num_long_blocks = data_total % info.num_blocks;
    let num_short_blocks = info.num_blocks - num_long_blocks;
    let short_len = data_total / info.num_blocks;
    let long_len = short_len + 1;

    let mut blocks: Vec<Vec<u8>> = (0..info.num_blocks)
        .map(|_| Vec::with_capacity(long_len + info.ecc_per_block))
        .collect();
    let mut block_conf: Vec<Vec<u8>> = (0..info.num_blocks)
        .map(|_| Vec::with_capacity(long_len + info.ecc_per_block))
        .collect();

    let mut idx = 0;
    for i in 0..long_len {
        for (b, block) in blocks.iter_mut().enumerate().take(info.num_blocks) {
            let block_len = if b < num_short_blocks {
                short_len
            } else {
                long_len
            };
            if i < block_len {
                if idx >= total {
                    return None;
                }
                block.push(codewords[idx]);
                if let Some(conf) = codeword_confidence {
                    block_conf[b].push(conf.get(idx).copied().unwrap_or(255));
                }
                idx += 1;
            }
        }
    }

    for _ in 0..info.ecc_per_block {
        for (b, block) in blocks.iter_mut().enumerate().take(info.num_blocks) {
            if idx >= total {
                return None;
            }
            block.push(codewords[idx]);
            if let Some(conf) = codeword_confidence {
                block_conf[b].push(conf.get(idx).copied().unwrap_or(255));
            }
            idx += 1;
        }
    }

    let rs = ReedSolomonDecoder::new(info.ecc_per_block);
    let mut data_out = Vec::with_capacity(data_total);
    for (b, block) in blocks.iter_mut().enumerate() {
        context.counters_mut().rs_block_attempts += 1;
        let mut corrected = rs.decode(block).is_ok();
        if !corrected {
            if let Some(conf) = codeword_confidence {
                let erasures = low_confidence_positions(
                    &block_conf[b],
                    if deterministic_erasures {
                        0
                    } else {
                        erasure_threshold()
                    },
                    if deterministic_erasures {
                        info.ecc_per_block
                    } else {
                        max_erasures_per_block(info.ecc_per_block)
                    },
                );
                if !erasures.is_empty() {
                    corrected = if deterministic_erasures {
                        rs.decode_with_erasures(block, &erasures).is_ok()
                    } else {
                        try_erasure_with_cap(&rs, block, &erasures, context)
                    };
                }
                let _ = conf;
            }
        }
        if !corrected {
            context.counters_mut().rs_block_failures += 1;
            return None;
        }
        context.counters_mut().rs_block_successes += 1;
        let data_len = if b < num_short_blocks {
            short_len
        } else {
            long_len
        };
        data_out.extend_from_slice(&block[..data_len]);
    }

    Some(data_out)
}

fn bits_to_codewords_with_confidence(
    bits: &[bool],
    bit_confidence: &[u8],
    msb: bool,
) -> (Vec<u8>, Vec<u8>) {
    let mut codewords = Vec::with_capacity(bits.len() / 8);
    let mut conf = Vec::with_capacity(bits.len() / 8);
    let mut idx = 0;
    while idx + 8 <= bits.len() {
        let mut byte = 0u8;
        let mut min_c = u8::MAX;
        for bit in 0..8 {
            if msb {
                byte = (byte << 1) | (bits[idx] as u8);
            } else if bits[idx] {
                byte |= 1 << bit;
            }
            if !bit_confidence.is_empty() {
                min_c = min_c.min(bit_confidence[idx]);
            }
            idx += 1;
        }
        codewords.push(byte);
        if !bit_confidence.is_empty() {
            conf.push(min_c);
        }
    }
    (codewords, conf)
}

fn erasure_threshold() -> u8 {
    crate::decoder::config::rs_erasure_conf_threshold()
}

fn max_erasures_per_block(ecc_per_block: usize) -> usize {
    let default_limit = (ecc_per_block / 2).max(1);
    match crate::decoder::config::rs_max_erasures_override() {
        Some(v) => v.min(ecc_per_block).max(1),
        None => default_limit,
    }
}

fn low_confidence_positions(confidence: &[u8], threshold: u8, max_count: usize) -> Vec<usize> {
    let mut indexed: Vec<(usize, u8)> = confidence
        .iter()
        .enumerate()
        .filter(|(_, c)| **c <= threshold)
        .map(|(i, c)| (i, *c))
        .collect();
    indexed.sort_by_key(|(i, c)| (*c, *i));
    indexed
        .into_iter()
        .take(max_count)
        .map(|(i, _)| i)
        .collect()
}

/// Check if RS erasure should be attempted based on global cap
/// Attempt RS erasure while consuming this request's bounded recovery budget.
fn try_erasure_with_cap(
    rs: &ReedSolomonDecoder,
    block: &mut [u8],
    erasures: &[usize],
    context: &mut DecodeRequestContext,
) -> bool {
    if !context.try_consume_erasure_attempt() {
        return false;
    }
    context.counters_mut().rs_erasure_attempts += 1;
    record_erasure_hist(context, erasures.len());
    if rs.decode_with_erasures(block, erasures).is_ok() {
        context.counters_mut().rs_erasure_successes += 1;
        return true;
    }
    false
}

#[cfg(test)]
mod telemetry_tests {
    use super::*;

    #[test]
    fn rs_block_failure_is_recorded_once_per_attempted_block() {
        // Version 1-L has a single 26-codeword block. Eight corrupt symbols
        // exceed its correction capacity, so this exercises a real RS miss
        // without relying on an image-level recovery route.
        let mut codewords = vec![0u8; 26];
        for (index, value) in codewords.iter_mut().take(8).enumerate() {
            *value = (index + 1) as u8;
        }
        let mut context = DecodeRequestContext::default();
        assert!(
            deinterleave_and_correct_with_confidence(
                &codewords,
                1,
                ECLevel::L,
                None,
                false,
                &mut context,
            )
            .is_none()
        );

        let counters = context.counters();
        assert_eq!(counters.rs_block_attempts, 1);
        assert_eq!(counters.rs_block_successes, 0);
        assert_eq!(counters.rs_block_failures, 1);
    }

    #[test]
    fn nonzero_remainder_bits_are_identified_without_touching_codewords() {
        assert!(!has_nonzero_remainder_bits(&[true; 16]));
        assert!(!has_nonzero_remainder_bits(&[
            true, true, true, true, true, true, true, true, true, true, true, true, true, true,
            true, true, false,
        ]));
        assert!(has_nonzero_remainder_bits(&[
            false, false, false, false, false, false, false, false, true,
        ]));
    }
}

#[allow(dead_code)]
pub(super) fn decode_payload(data_codewords: &[u8], version: u8) -> Option<(Vec<u8>, String)> {
    let mut bits = Vec::with_capacity(data_codewords.len() * 8);
    for &byte in data_codewords {
        for i in (0..8).rev() {
            bits.push(((byte >> i) & 1) != 0);
        }
    }

    let decoded = decode_payload_from_bits_with_tail(&bits, version)?;
    Some((decoded.data, decoded.content))
}

#[allow(dead_code)]
pub(super) fn decode_payload_with_metadata(
    data_codewords: &[u8],
    version: u8,
) -> Option<(Vec<u8>, String, QRCodeMetadata)> {
    let mut bits = Vec::with_capacity(data_codewords.len() * 8);
    for &byte in data_codewords {
        for i in (0..8).rev() {
            bits.push(((byte >> i) & 1) != 0);
        }
    }
    let decoded = decode_payload_from_bits_with_tail(&bits, version)?;
    Some((decoded.data, decoded.content, decoded.metadata))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PayloadDecodeError {
    Malformed,
    UnsupportedMode(u8),
}

fn decode_payload_strict_result(
    data_codewords: &[u8],
    version: u8,
) -> Result<DecodedPayload, PayloadDecodeError> {
    let mut bits = Vec::with_capacity(data_codewords.len() * 8);
    for &byte in data_codewords {
        for i in (0..8).rev() {
            bits.push(((byte >> i) & 1) != 0);
        }
    }
    let decoded = decode_payload_from_bits_with_tail_result(&bits, version)?;
    valid_terminator_and_padding(&bits, decoded.tail_start)
        .then_some(decoded)
        .ok_or(PayloadDecodeError::Malformed)
}

#[allow(dead_code)]
pub(super) fn decode_payload_from_bits(bits: &[bool], version: u8) -> Option<(Vec<u8>, String)> {
    let decoded = decode_payload_from_bits_with_tail(bits, version)?;
    Some((decoded.data, decoded.content))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DecodedPayload {
    data: Vec<u8>,
    content: String,
    metadata: QRCodeMetadata,
    tail_start: usize,
}

fn decode_payload_from_bits_with_tail(bits: &[bool], version: u8) -> Option<DecodedPayload> {
    decode_payload_from_bits_with_tail_result(bits, version).ok()
}

fn decode_payload_from_bits_with_tail_result(
    bits: &[bool],
    version: u8,
) -> Result<DecodedPayload, PayloadDecodeError> {
    let mut reader = BitReader::new(bits);
    let mut data = Vec::new();
    let mut content = String::new();
    let mut metadata = QRCodeMetadata::default();

    loop {
        if reader.remaining() < 4 {
            break;
        }
        let mode_start = reader.index();
        let mode = reader.read_bits(4).ok_or(PayloadDecodeError::Malformed)? as u8;
        if mode == 0 {
            reader.idx = mode_start;
            break;
        }

        match mode {
            1 => {
                let count_bits = char_count_bits(mode, version);
                let count = reader
                    .read_bits(count_bits)
                    .ok_or(PayloadDecodeError::Malformed)? as usize;
                let start = reader.index();
                let (decoded, used) = NumericDecoder::decode(&bits[start..], count)
                    .ok_or(PayloadDecodeError::Malformed)?;
                reader.advance(used);
                data.extend_from_slice(decoded.as_bytes());
                content.push_str(&decoded);
            }
            2 => {
                let count_bits = char_count_bits(mode, version);
                let count = reader
                    .read_bits(count_bits)
                    .ok_or(PayloadDecodeError::Malformed)? as usize;
                let start = reader.index();
                let (decoded, used) = AlphanumericDecoder::decode(&bits[start..], count)
                    .ok_or(PayloadDecodeError::Malformed)?;
                reader.advance(used);
                let decoded = apply_fnc1_substitution(&decoded, metadata.fnc1.is_some());
                data.extend_from_slice(decoded.as_bytes());
                content.push_str(&decoded);
            }
            4 => {
                let count_bits = char_count_bits(mode, version);
                let count = reader
                    .read_bits(count_bits)
                    .ok_or(PayloadDecodeError::Malformed)? as usize;
                let mut bytes = Vec::with_capacity(count);
                for _ in 0..count {
                    let byte = reader.read_bits(8).ok_or(PayloadDecodeError::Malformed)? as u8;
                    bytes.push(byte);
                }
                data.extend_from_slice(&bytes);
                content.push_str(&String::from_utf8_lossy(&bytes));
            }
            7 => {
                metadata.eci_assignment =
                    Some(read_eci_assignment(&mut reader).ok_or(PayloadDecodeError::Malformed)?);
            }
            8 => {
                let count_bits = char_count_bits(mode, version);
                let count = reader
                    .read_bits(count_bits)
                    .ok_or(PayloadDecodeError::Malformed)? as usize;
                let mut sjis_bytes = Vec::with_capacity(count * 2);
                let start = reader.index();
                let (decoded, used) = KanjiDecoder::decode(&bits[start..], count)
                    .ok_or(PayloadDecodeError::Malformed)?;
                reader.advance(used);
                sjis_bytes.extend_from_slice(&decoded);
                data.extend_from_slice(&sjis_bytes);
                content.push_str(&String::from_utf8_lossy(&sjis_bytes));
            }
            3 => {
                let index = reader.read_bits(4).ok_or(PayloadDecodeError::Malformed)? as u8;
                let total_symbols =
                    reader.read_bits(4).ok_or(PayloadDecodeError::Malformed)? as u8 + 1;
                let parity = reader.read_bits(8).ok_or(PayloadDecodeError::Malformed)? as u8;
                if metadata.structured_append.is_some() {
                    return Err(PayloadDecodeError::Malformed);
                }
                metadata.structured_append = Some(StructuredAppendInfo {
                    index,
                    total_symbols,
                    parity,
                });
            }
            5 => {
                if metadata.fnc1.is_some() {
                    return Err(PayloadDecodeError::Malformed);
                }
                metadata.fnc1 = Some(Fnc1Position::First);
            }
            9 => {
                if metadata.fnc1.is_some() {
                    return Err(PayloadDecodeError::Malformed);
                }
                metadata.fnc1 = Some(Fnc1Position::Second {
                    application_indicator: reader
                        .read_bits(8)
                        .ok_or(PayloadDecodeError::Malformed)?
                        as u8,
                });
            }
            _ => return Err(PayloadDecodeError::UnsupportedMode(mode)),
        }
    }

    Ok(DecodedPayload {
        data,
        content,
        metadata,
        tail_start: reader.index(),
    })
}

fn read_eci_assignment(reader: &mut BitReader<'_>) -> Option<u32> {
    let first = reader.read_bits(8)?;
    if first & 0x80 == 0 {
        Some(first)
    } else if first & 0xc0 == 0x80 {
        Some(((first & 0x3f) << 8) | reader.read_bits(8)?)
    } else if first & 0xe0 == 0xc0 {
        Some(((first & 0x1f) << 16) | reader.read_bits(16)?)
    } else {
        None
    }
}

fn apply_fnc1_substitution(input: &str, fnc1_active: bool) -> String {
    if !fnc1_active {
        return input.to_owned();
    }
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '%' {
            if chars.peek() == Some(&'%') {
                chars.next();
                result.push('%');
            } else {
                result.push('\u{1d}');
            }
        } else {
            result.push(character);
        }
    }
    result
}

fn valid_terminator_and_padding(bits: &[bool], tail_start: usize) -> bool {
    let terminator_end = (tail_start + 4).min(bits.len());
    if bits[tail_start..terminator_end].iter().any(|bit| *bit) {
        return false;
    }
    let byte_boundary = ((terminator_end + 7) / 8 * 8).min(bits.len());
    if bits[terminator_end..byte_boundary].iter().any(|bit| *bit) {
        return false;
    }
    bits[byte_boundary..]
        .chunks_exact(8)
        .enumerate()
        .all(|(index, byte)| {
            let value = byte
                .iter()
                .fold(0u8, |value, bit| (value << 1) | u8::from(*bit));
            value == if index % 2 == 0 { 0xec } else { 0x11 }
        })
}

struct BitReader<'a> {
    bits: &'a [bool],
    idx: usize,
}

impl<'a> BitReader<'a> {
    fn new(bits: &'a [bool]) -> Self {
        Self { bits, idx: 0 }
    }

    fn remaining(&self) -> usize {
        self.bits.len().saturating_sub(self.idx)
    }

    fn index(&self) -> usize {
        self.idx
    }

    fn advance(&mut self, n: usize) {
        self.idx = (self.idx + n).min(self.bits.len());
    }

    fn read_bits(&mut self, n: usize) -> Option<u32> {
        if self.idx + n > self.bits.len() {
            return None;
        }
        let mut val = 0u32;
        for _ in 0..n {
            val = (val << 1) | (self.bits[self.idx] as u32);
            self.idx += 1;
        }
        Some(val)
    }
}

fn char_count_bits(mode: u8, version: u8) -> usize {
    let ver = version as usize;
    match mode {
        1 => {
            if ver <= 9 {
                10
            } else if ver <= 26 {
                12
            } else {
                14
            }
        }
        2 => {
            if ver <= 9 {
                9
            } else if ver <= 26 {
                11
            } else {
                13
            }
        }
        4 => {
            if ver <= 9 {
                8
            } else {
                16
            }
        }
        8 => {
            if ver <= 9 {
                8
            } else if ver <= 26 {
                10
            } else {
                12
            }
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidence_to_erasure_mapping_is_ordered_and_bounded() {
        let conf = vec![120, 10, 30, 250, 20, 40];
        let erasures = low_confidence_positions(&conf, 40, 3);
        assert_eq!(erasures, vec![1, 4, 2]);
    }

    #[test]
    fn bits_to_codewords_confidence_tracks_min_bit_confidence() {
        let bits = vec![
            true, false, true, false, true, false, true, false, false, false, false, false, false,
            false, false, false,
        ];
        let conf = vec![
            80, 70, 60, 50, 40, 30, 20, 10, 255, 255, 255, 255, 255, 255, 255, 255,
        ];
        let (cw, cc) = bits_to_codewords_with_confidence(&bits, &conf, true);
        assert_eq!(cw.len(), 2);
        assert_eq!(cc, vec![10, 255]);
    }

    #[test]
    fn payload_reports_reserved_mode_as_unsupported() {
        let bits = [false, true, true, false]; // Reserved mode 0110.
        assert_eq!(
            decode_payload_from_bits_with_tail_result(&bits, 1),
            Err(PayloadDecodeError::UnsupportedMode(6))
        );
    }
}

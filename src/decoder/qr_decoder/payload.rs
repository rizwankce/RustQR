use crate::decoder::bitstream::BitstreamExtractor;
use crate::decoder::format::FormatInfo;
use crate::decoder::function_mask::FunctionMask;
use crate::decoder::modes::{alphanumeric::AlphanumericDecoder, numeric::NumericDecoder};
use crate::decoder::reed_solomon::ReedSolomonDecoder;
use crate::decoder::tables::ec_block_info;
use crate::decoder::unmask::unmask;
use crate::decoder::version::VersionInfo;
use crate::models::{BitMatrix, ECLevel, QRCode, Version};
use std::cell::RefCell;

#[derive(Clone, Copy, Default)]
struct ErasureCounters {
    attempts: usize,
    successes: usize,
    hist_1: usize,
    hist_2_3: usize,
    hist_4_6: usize,
    hist_7_plus: usize,
}

thread_local! {
    static ERASURE_COUNTERS: RefCell<ErasureCounters> = const { RefCell::new(ErasureCounters {
        attempts: 0,
        successes: 0,
        hist_1: 0,
        hist_2_3: 0,
        hist_4_6: 0,
        hist_7_plus: 0,
    }) };
}

/// Global counter for RS erasure attempts (across all blocks in an image)
static RS_ERASURE_GLOBAL_ATTEMPTS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

pub(crate) fn reset_rs_erasure_global_counter() {
    RS_ERASURE_GLOBAL_ATTEMPTS.store(0, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn get_rs_erasure_global_counter() -> usize {
    RS_ERASURE_GLOBAL_ATTEMPTS.load(std::sync::atomic::Ordering::Relaxed)
}

pub(crate) fn increment_rs_erasure_global_counter() -> usize {
    RS_ERASURE_GLOBAL_ATTEMPTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

pub(super) fn reset_erasure_counters() {
    ERASURE_COUNTERS.with(|c| *c.borrow_mut() = ErasureCounters::default());
}

pub(super) fn take_erasure_counters() -> (usize, usize, [usize; 4]) {
    ERASURE_COUNTERS.with(|c| {
        let ec = *c.borrow();
        *c.borrow_mut() = ErasureCounters::default();
        (
            ec.attempts,
            ec.successes,
            [ec.hist_1, ec.hist_2_3, ec.hist_4_6, ec.hist_7_plus],
        )
    })
}

fn record_erasure_hist(count: usize) {
    ERASURE_COUNTERS.with(|c| {
        let mut ec = c.borrow_mut();
        match count {
            0 => {}
            1 => ec.hist_1 += 1,
            2..=3 => ec.hist_2_3 += 1,
            4..=6 => ec.hist_4_6 += 1,
            _ => ec.hist_7_plus += 1,
        }
    });
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

    let data_codewords = deinterleave_and_correct_with_confidence(
        &codewords,
        version_num,
        format_info.ec_level,
        if codeword_confidence.is_empty() {
            None
        } else {
            Some(&codeword_confidence)
        },
    )?;

    let (data, content) = decode_payload(&data_codewords, version_num)?;
    if data.is_empty() {
        return None;
    }

    let version = if dimension >= 45 {
        VersionInfo::extract(oriented)
            .map(Version::Model2)
            .unwrap_or(Version::Model2(version_num))
    } else {
        Version::Model2(version_num)
    };

    Some(QRCode::new(
        data,
        content,
        version,
        format_info.ec_level,
        format_info.mask_pattern,
    ))
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
    deinterleave_and_correct_with_confidence(codewords, version, ec_level, None)
}

pub(super) fn deinterleave_and_correct_with_confidence(
    codewords: &[u8],
    version: u8,
    ec_level: ECLevel,
    codeword_confidence: Option<&[u8]>,
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
        let mut corrected = rs.decode(block).is_ok();
        if !corrected {
            if let Some(conf) = codeword_confidence {
                let erasures = low_confidence_positions(
                    &block_conf[b],
                    erasure_threshold(),
                    max_erasures_per_block(info.ecc_per_block),
                );
                if !erasures.is_empty() {
                    corrected = try_erasure_with_cap(&rs, block, &erasures);
                }
                let _ = conf;
            }
        }
        if !corrected {
            return None;
        }
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
fn should_attempt_erasure() -> bool {
    let global_cap = crate::decoder::config::rs_erasure_global_cap();
    if global_cap == 0 {
        return true; // No cap
    }
    let current = get_rs_erasure_global_counter();
    if current >= global_cap {
        return false; // Cap reached
    }
    true
}

/// Attempt RS erasure with global cap tracking
fn try_erasure_with_cap(rs: &ReedSolomonDecoder, block: &mut [u8], erasures: &[usize]) -> bool {
    if !should_attempt_erasure() {
        return false;
    }
    let current = increment_rs_erasure_global_counter();
    if current > crate::decoder::config::rs_erasure_global_cap() {
        return false;
    }
    ERASURE_COUNTERS.with(|c| c.borrow_mut().attempts += 1);
    record_erasure_hist(erasures.len());
    if rs.decode_with_erasures(block, erasures).is_ok() {
        ERASURE_COUNTERS.with(|c| c.borrow_mut().successes += 1);
        return true;
    }
    false
}

pub(super) fn decode_payload(data_codewords: &[u8], version: u8) -> Option<(Vec<u8>, String)> {
    let mut bits = Vec::with_capacity(data_codewords.len() * 8);
    for &byte in data_codewords {
        for i in (0..8).rev() {
            bits.push(((byte >> i) & 1) != 0);
        }
    }

    decode_payload_from_bits(&bits, version)
}

pub(super) fn decode_payload_from_bits(bits: &[bool], version: u8) -> Option<(Vec<u8>, String)> {
    let mut reader = BitReader::new(bits);
    let mut data = Vec::new();
    let mut content = String::new();

    loop {
        if reader.remaining() < 4 {
            break;
        }
        let mode = reader.read_bits(4)? as u8;
        if mode == 0 {
            break;
        }

        match mode {
            1 => {
                let count_bits = char_count_bits(mode, version);
                let count = reader.read_bits(count_bits)? as usize;
                let start = reader.index();
                let (decoded, used) = NumericDecoder::decode(&bits[start..], count)?;
                reader.advance(used);
                data.extend_from_slice(decoded.as_bytes());
                content.push_str(&decoded);
            }
            2 => {
                let count_bits = char_count_bits(mode, version);
                let count = reader.read_bits(count_bits)? as usize;
                let start = reader.index();
                let (decoded, used) = AlphanumericDecoder::decode(&bits[start..], count)?;
                reader.advance(used);
                data.extend_from_slice(decoded.as_bytes());
                content.push_str(&decoded);
            }
            4 => {
                let count_bits = char_count_bits(mode, version);
                let count = reader.read_bits(count_bits)? as usize;
                let mut bytes = Vec::with_capacity(count);
                for _ in 0..count {
                    let byte = reader.read_bits(8)? as u8;
                    bytes.push(byte);
                }
                data.extend_from_slice(&bytes);
                content.push_str(&String::from_utf8_lossy(&bytes));
            }
            7 => {
                // ECI: parse and ignore for now (assume UTF-8)
                let mut eci = reader.read_bits(8)?;
                if (eci & 0x80) != 0 {
                    eci = ((eci & 0x7F) << 8) | reader.read_bits(8)?;
                    if (eci & 0x4000) != 0 {
                        eci = ((eci & 0x3FFF) << 8) | reader.read_bits(8)?;
                    }
                }
                let _ = eci;
            }
            8 => {
                // Kanji mode: decode Shift-JIS code units from 13-bit values.
                // We preserve bytes in `data` and append a lossy textual representation.
                let count_bits = char_count_bits(mode, version);
                let count = reader.read_bits(count_bits)? as usize;
                let mut sjis_bytes = Vec::with_capacity(count * 2);
                for _ in 0..count {
                    let val = reader.read_bits(13)? as u16;
                    let mut intermediate = ((val / 0xC0) << 8) | (val % 0xC0);
                    if intermediate < 0x1F00 {
                        intermediate += 0x8140;
                    } else {
                        intermediate += 0xC140;
                    }
                    sjis_bytes.push((intermediate >> 8) as u8);
                    sjis_bytes.push((intermediate & 0xFF) as u8);
                }
                data.extend_from_slice(&sjis_bytes);
                content.push_str(&String::from_utf8_lossy(&sjis_bytes));
            }
            _ => return None,
        }
    }

    Some((data, content))
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
}

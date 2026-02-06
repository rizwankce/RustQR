use crate::decoder::bitstream::BitstreamExtractor;
use crate::decoder::format::FormatInfo;
use crate::decoder::function_mask::FunctionMask;
use crate::decoder::modes::{alphanumeric::AlphanumericDecoder, numeric::NumericDecoder};
use crate::decoder::reed_solomon::ReedSolomonDecoder;
use crate::decoder::tables::ec_block_info;
use crate::decoder::unmask::unmask;
use crate::decoder::version::VersionInfo;
use crate::models::{BitMatrix, ECLevel, QRCode, Version};

pub(super) fn try_decode_single(
    oriented: &BitMatrix,
    version_num: u8,
    format_info: &FormatInfo,
    start_upward: bool,
    swap_columns: bool,
    use_msb: bool,
    reverse_stream: bool,
) -> Option<QRCode> {
    let dimension = oriented.width();
    let func = FunctionMask::new(version_num);
    let mut unmasked = oriented.clone();
    unmask(&mut unmasked, &format_info.mask_pattern, &func);

    let bits = BitstreamExtractor::extract_with_options(
        &unmasked,
        dimension,
        &func,
        start_upward,
        swap_columns,
    );

    let bits = if reverse_stream {
        let mut rev = bits;
        rev.reverse();
        rev
    } else {
        bits
    };

    let codewords = if use_msb {
        bits_to_codewords(&bits)
    } else {
        bits_to_codewords_lsb(&bits)
    };

    let data_codewords = deinterleave_and_correct(&codewords, version_num, format_info.ec_level)?;

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

pub(super) fn deinterleave_and_correct(
    codewords: &[u8],
    version: u8,
    ec_level: ECLevel,
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
                idx += 1;
            }
        }
    }

    for _ in 0..info.ecc_per_block {
        for block in blocks.iter_mut().take(info.num_blocks) {
            if idx >= total {
                return None;
            }
            block.push(codewords[idx]);
            idx += 1;
        }
    }

    let rs = ReedSolomonDecoder::new(info.ecc_per_block);
    let mut data_out = Vec::with_capacity(data_total);
    for (b, block) in blocks.iter_mut().enumerate() {
        if rs.decode(block).is_err() {
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

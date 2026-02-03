use crate::models::ECLevel;

pub struct EcBlockInfo {
    pub num_blocks: usize,
    pub ecc_per_block: usize,
}

// Tables from the QR Code specification (Model 2) via Nayuki QR Code generator.
// Index: [ec_level][version]
const ECC_CODEWORDS_PER_BLOCK: [[i8; 41]; 4] = [
    [
        -1, 7, 10, 15, 20, 26, 18, 20, 24, 30, 18, 20, 24, 26, 30, 22, 24, 28, 30, 28, 28, 28, 28,
        30, 30, 26, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30,
    ], // Low
    [
        -1, 10, 16, 26, 18, 24, 16, 18, 22, 22, 26, 30, 22, 22, 24, 24, 28, 28, 26, 26, 26, 26, 28,
        28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28,
    ], // Medium
    [
        -1, 13, 22, 18, 26, 18, 24, 18, 22, 20, 24, 28, 26, 24, 20, 30, 24, 28, 28, 26, 30, 28, 30,
        30, 30, 30, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30,
    ], // Quartile
    [
        -1, 17, 28, 22, 16, 22, 28, 26, 26, 24, 28, 24, 28, 22, 24, 24, 30, 28, 28, 26, 28, 30, 24,
        30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30,
    ], // High
];

const NUM_ERROR_CORRECTION_BLOCKS: [[i8; 41]; 4] = [
    [
        -1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 4, 4, 4, 4, 4, 6, 6, 6, 6, 7, 8, 8, 9, 9, 10, 12, 12, 12,
        13, 14, 15, 16, 17, 18, 19, 19, 20, 21, 22, 24, 25,
    ], // Low
    [
        -1, 1, 1, 1, 2, 2, 4, 4, 4, 5, 5, 5, 8, 9, 9, 10, 10, 11, 13, 14, 16, 17, 17, 18, 20, 21,
        23, 25, 26, 28, 29, 31, 33, 35, 37, 38, 40, 43, 45, 47, 49,
    ], // Medium
    [
        -1, 1, 1, 2, 2, 4, 4, 6, 6, 8, 8, 8, 10, 12, 16, 12, 17, 16, 18, 21, 20, 23, 23, 25, 27,
        29, 34, 34, 35, 38, 40, 43, 45, 48, 51, 53, 56, 59, 62, 65, 68,
    ], // Quartile
    [
        -1, 1, 1, 2, 4, 4, 4, 5, 6, 8, 8, 11, 11, 16, 16, 18, 16, 19, 21, 25, 25, 25, 34, 30, 32,
        35, 37, 40, 42, 45, 48, 51, 54, 57, 60, 63, 66, 70, 74, 77, 81,
    ], // High
];

pub fn ec_block_info(version: u8, ec_level: ECLevel) -> Option<EcBlockInfo> {
    if !(1..=40).contains(&version) {
        return None;
    }
    let idx = ec_level_index(ec_level);
    let ecc = ECC_CODEWORDS_PER_BLOCK[idx][version as usize];
    let blocks = NUM_ERROR_CORRECTION_BLOCKS[idx][version as usize];
    if ecc <= 0 || blocks <= 0 {
        return None;
    }
    Some(EcBlockInfo {
        num_blocks: blocks as usize,
        ecc_per_block: ecc as usize,
    })
}

fn ec_level_index(ec_level: ECLevel) -> usize {
    match ec_level {
        ECLevel::L => 0,
        ECLevel::M => 1,
        ECLevel::Q => 2,
        ECLevel::H => 3,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QrCode {
    pub payload: String,
    pub confidence: f32,
    pub corners: [Point; 4],
}

impl QrCode {
    pub fn new(payload: String, confidence: f32, corners: [Point; 4]) -> Self {
        Self {
            payload,
            confidence,
            corners,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Proposal {
    pub id: usize,
    pub score: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hypothesis {
    pub id: usize,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecodeCandidate {
    pub score: f32,
    pub qr: QrCode,
}

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
    pub view: ProposalView,
    pub x: usize,
    pub y: usize,
    pub score: f32,
    pub raw_score: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProposalView {
    Otsu,
    Adaptive,
    Sauvola,
    GlareSuppressed,
}

impl ProposalView {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Otsu => "otsu",
            Self::Adaptive => "adaptive",
            Self::Sauvola => "sauvola",
            Self::GlareSuppressed => "glare_suppressed",
        }
    }
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

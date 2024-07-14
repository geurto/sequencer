#[derive(Clone, Debug)]
pub struct MarkovSequencerConfig {
    pub root_note: u8,
    pub steps: usize,
}

impl MarkovSequencerConfig {
    pub fn new() -> Self {
        Self {
            root_note: 60,
            steps: 16,
        }
    }
}
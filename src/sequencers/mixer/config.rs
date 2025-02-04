use crate::note::Sequence;

use log::debug;

#[derive(Debug)]
pub struct MixerConfig {
    pub ratio: f32,
    pub sequence_a: Sequence,
    pub sequence_b: Sequence,
    pub mixed_sequence: Sequence,
}

impl MixerConfig {
    pub fn new() -> Self {
        MixerConfig {
            ratio: 0.5,
            sequence_a: Sequence::empty(),
            sequence_b: Sequence::empty(),
            mixed_sequence: Sequence::empty(),
        }
    }

    pub fn increase_ratio(&mut self) {
        self.ratio = (self.ratio + 0.05).clamp(0.0, 1.0);
        debug!("Mixer ratio increased to {}", self.ratio);
    }

    pub fn decrease_ratio(&mut self) {
        self.ratio = (self.ratio - 0.05).clamp(0.0, 1.0);
        debug!("Mixer ratio decreased to {}", self.ratio);
    }

    pub fn update_sequence_a(&mut self, sequence: Sequence) {
        self.sequence_a = sequence;
        debug!("Updated sequence A");
    }

    pub fn update_sequence_b(&mut self, sequence: Sequence) {
        self.sequence_b = sequence;
        debug!("Updated sequence B");
    }
}

impl Default for MixerConfig {
    fn default() -> Self {
        Self::new()
    }
}


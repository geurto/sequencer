use log::debug;

pub enum MixerInput {
    IncreaseRatio,
    DecreaseRatio,
}

#[derive(Clone, Debug)]
pub struct MixerState {
    pub ratio: f32,
}

impl MixerState {
    pub fn new() -> Self {
        MixerState { ratio: 0.5 }
    }

    pub fn increase_ratio(&mut self) {
        self.ratio = (self.ratio + 0.05).clamp(0.0, 1.0);
        debug!("Mixer ratio increased to {}", self.ratio);
    }

    pub fn decrease_ratio(&mut self) {
        self.ratio = (self.ratio - 0.05).clamp(0.0, 1.0);
        debug!("Mixer ratio decreased to {}", self.ratio);
    }
}

impl Default for MixerState {
    fn default() -> Self {
        Self::new()
    }
}

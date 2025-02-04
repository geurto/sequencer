use log::debug;

#[derive(Clone, Debug)]
pub struct EuclideanSequencerConfig {
    pub steps: usize,
    pub pulses: usize,
    pub phase: usize,
    pub pitch: u8,
}

impl EuclideanSequencerConfig {
    pub fn new() -> Self {
        EuclideanSequencerConfig {
            steps: 16,
            pulses: 0,
            phase: 0,
            pitch: 60,
        }
    }

    pub fn increase_steps(&mut self) {
        if self.steps < 16 {
            self.steps += 1;
        }
        debug!("Steps: {}", self.steps);
    }

    pub fn decrease_steps(&mut self) {
        if self.steps > 1 {
            self.steps -= 1;
        }
        debug!("Steps: {}", self.steps);
    }

    pub fn increase_pulses(&mut self) {
        if self.pulses < 16 {
            self.pulses += 1;
        }
        debug!("Pulses: {}", self.pulses);
    }

    pub fn decrease_pulses(&mut self) {
        if self.pulses > 1 {
            self.pulses -= 1;
        }
        debug!("Pulses: {}", self.pulses);
    }

    pub fn increase_phase(&mut self) {
        self.phase = (self.phase + 1) % self.steps;
        debug!("Phase: {}", self.phase);
    }

    pub fn decrease_phase(&mut self) {
        self.phase = (self.phase - 1) % self.steps;
        debug!("Phase: {}", self.phase);
    }

    pub fn change_pitch(&mut self, amount: i8) {
        self.pitch = (self.pitch as i8 + amount) as u8;
        self.pitch = self.pitch.clamp(20, 108);
        debug!("Pitch: {}", self.pitch);
    }
}

impl Default for EuclideanSequencerConfig {
    fn default() -> Self {
        Self::new()
    }
}

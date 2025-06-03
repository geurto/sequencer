use log::info;

pub enum EuclideanSequencerInput {
    IncreaseSteps,
    DecreaseSteps,
    IncreasePulses,
    DecreasePulses,
    IncreasePhase,
    DecreasePhase,
    IncreasePitch,
    DecreasePitch,
    IncreaseOctave,
    DecreaseOctave,
}

#[derive(Clone, Copy, Debug)]
pub struct EuclideanSequencerState {
    pub steps: usize,
    pub pulses: usize,
    pub phase: usize,
    pub pitch: u8,
}

impl EuclideanSequencerState {
    pub fn new() -> Self {
        EuclideanSequencerState {
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
        info!("Steps: {}", self.steps);
    }

    pub fn decrease_steps(&mut self) {
        if self.steps > 1 {
            self.steps -= 1;
        }
        info!("Steps: {}", self.steps);
    }

    pub fn increase_pulses(&mut self) {
        if self.pulses < 16 {
            self.pulses += 1;
        }
        info!("Pulses: {}", self.pulses);
    }

    pub fn decrease_pulses(&mut self) {
        if self.pulses > 0 {
            self.pulses -= 1;
        }
        info!("Pulses: {}", self.pulses);
    }

    pub fn increase_phase(&mut self) {
        self.phase = (self.phase + 1) % self.steps;
        info!("Phase: {}", self.phase);
    }

    pub fn decrease_phase(&mut self) {
        self.phase = (self.phase - 1) % self.steps;
        info!("Phase: {}", self.phase);
    }

    pub fn change_pitch(&mut self, amount: i8) {
        self.pitch = (self.pitch as i8 + amount) as u8;
        self.pitch = self.pitch.clamp(20, 108);
        info!("Pitch: {}", self.pitch);
    }
}

impl Default for EuclideanSequencerState {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for EuclideanSequencerState {
    fn eq(&self, other: &Self) -> bool {
        self.steps == other.steps
            && self.pulses == other.pulses
            && self.phase == other.phase
            && self.pitch == other.pitch
    }
}

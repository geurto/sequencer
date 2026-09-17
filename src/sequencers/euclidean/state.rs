use log::info;

pub const MAX_STEPS: usize = 16;
pub const MIN_PITCH: u8 = 20;
pub const MAX_PITCH: u8 = 108;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EuclideanSequencerState {
    pub steps: usize,
    pub pulses: usize,
    pub phase: usize,
    pub pitch: u8,
}

impl EuclideanSequencerState {
    pub fn new() -> Self {
        EuclideanSequencerState {
            steps: MAX_STEPS,
            // Four on the floor, so the sequencer makes a sound on startup
            // rather than waiting for the user to find the pulse keys.
            pulses: 4,
            phase: 0,
            pitch: 60,
        }
    }

    pub fn increase_steps(&mut self) {
        if self.steps < MAX_STEPS {
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
        if self.pulses < MAX_STEPS {
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
        // Wrap to the far end, mirroring increase_phase. `saturating_sub` would
        // make phase 0 a dead stop in one direction only.
        self.phase = (self.phase + self.steps - 1) % self.steps;
        info!("Phase: {}", self.phase);
    }

    pub fn change_pitch(&mut self, amount: i8) {
        // Widen before adding: `self.pitch as i8` wraps for any pitch above
        // 127, and the addition itself can overflow i8.
        let pitch = i16::from(self.pitch) + i16::from(amount);
        self.pitch =
            pitch.clamp(i16::from(MIN_PITCH), i16::from(MAX_PITCH)) as u8;
        info!("Pitch: {}", self.pitch);
    }
}

impl Default for EuclideanSequencerState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_wraps_in_both_directions() {
        let mut state = EuclideanSequencerState::new();
        state.steps = 8;
        state.phase = 0;

        state.decrease_phase();
        assert_eq!(state.phase, 7, "decreasing past zero should wrap");

        state.increase_phase();
        assert_eq!(state.phase, 0, "increasing past the end should wrap");
    }

    #[test]
    fn test_pitch_stays_in_range() {
        let mut state = EuclideanSequencerState::new();

        for _ in 0..32 {
            state.change_pitch(12);
        }
        assert_eq!(state.pitch, MAX_PITCH);

        for _ in 0..32 {
            state.change_pitch(-12);
        }
        assert_eq!(state.pitch, MIN_PITCH);
    }

    #[test]
    fn test_pitch_change_never_wraps() {
        for start in MIN_PITCH..=MAX_PITCH {
            for amount in [i8::MIN, -12, -1, 1, 12, i8::MAX] {
                let mut state = EuclideanSequencerState::new();
                state.pitch = start;
                state.change_pitch(amount);
                assert!(
                    (MIN_PITCH..=MAX_PITCH).contains(&state.pitch),
                    "pitch {start} + {amount} left range at {}",
                    state.pitch
                );
            }
        }
    }
}

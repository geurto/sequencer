use super::Sequencer;
use crate::common::Sequence;

pub struct EuclideanSequencer {
    steps: usize,
    pulses: usize,
    pitch: u8,
}

impl EuclideanSequencer {
    pub fn new(steps: usize, pulses: usize, pitch: u8) -> Self {
        EuclideanSequencer {
            steps,
            pulses,
            pitch,
        }
    }
}

impl Sequencer for EuclideanSequencer {
    fn generate_sequence(&self, length: usize) -> Sequence {
        let mut sequence = Sequence {
            pitch: vec![],
            velocity: vec![100; length],
            duration: vec![500; length],
        };

        if self.pulses == 0 {
            // Handle zero pulses case
            sequence.pitch = vec![0; length];
            return sequence;
        }

        // Bresenham line algorithm cus it looks easier
        let beat_locations = (0..self.pulses)
            .map(|i| (i * self.steps) / self.pulses)
            .collect::<Vec<_>>();

        for i in 0..length {
            let note_pitch = if beat_locations.contains(&(i % self.steps)) {
                self.pitch
            } else {
                0
            };
            sequence.pitch.push(note_pitch);
        }
        sequence
    }
}
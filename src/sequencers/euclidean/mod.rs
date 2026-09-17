pub mod state;

use std::sync::Arc;
use std::time::Duration;

use crate::sequencers::euclidean::state::EuclideanSequencerState;
use crate::sequencers::{Note, NoteDuration, Sequence, Sequencer};

use crate::playback::state::{SequencerSlot, SharedState};
use log::{debug, error};
use tokio::sync::{mpsc, RwLock};

/// Velocity of a struck pulse. The mixer scales this by the crossfade ratio.
const PULSE_VELOCITY: u8 = 100;

pub struct EuclideanSequencer {
    sequencer_slot: SequencerSlot,
    state: EuclideanSequencerState,
    shared_state: Arc<RwLock<SharedState>>,
    tx_sequence: mpsc::Sender<(Option<Sequence>, Option<Sequence>)>,
}

impl EuclideanSequencer {
    pub fn new(
        sequencer_slot: SequencerSlot,
        shared_state: Arc<RwLock<SharedState>>,
        tx_sequence: mpsc::Sender<(Option<Sequence>, Option<Sequence>)>,
    ) -> Self {
        EuclideanSequencer {
            sequencer_slot,
            state: EuclideanSequencerState::new(),
            shared_state,
            tx_sequence,
        }
    }

    async fn read_shared_state(&self) -> EuclideanSequencerState {
        let shared = self.shared_state.read().await;
        match self.sequencer_slot {
            SequencerSlot::Left => shared.left_sequencer,
            SequencerSlot::Right => shared.right_sequencer,
        }
    }

    async fn send_sequence(&self) {
        debug!("Sending {:?} sequence to mixer", self.sequencer_slot);
        let sequence = self.generate_sequence();
        let payload = match self.sequencer_slot {
            SequencerSlot::Left => (Some(sequence), None),
            SequencerSlot::Right => (None, Some(sequence)),
        };
        if let Err(e) = self.tx_sequence.send(payload).await {
            error!("Error sending {:?} Sequence: {e}", self.sequencer_slot);
        }
    }
}

impl Sequencer for EuclideanSequencer {
    fn generate_sequence(&self) -> Sequence {
        if self.state.pulses == 0 {
            // Handle zero pulses case
            return Sequence {
                notes: vec![Note::rest(); self.state.steps],
            };
        }

        let mut sequence = Sequence::empty();

        // Bresenham line algorithm cus it looks easier
        let beat_locations = (0..self.state.pulses)
            .map(|i| {
                (self.state.phase + (i * self.state.steps) / self.state.pulses)
                    % self.state.steps
            })
            .collect::<Vec<_>>();

        for i in 0..self.state.steps {
            let note = if beat_locations.contains(&(i % self.state.steps)) {
                Note::new(
                    self.state.pitch,
                    PULSE_VELOCITY,
                    NoteDuration::Sixteenth,
                )
            } else {
                Note::rest()
            };
            sequence.notes.push(note);
        }
        sequence
    }

    async fn run(&mut self) {
        // Emit once up front. The loop below only reacts to *changes*, and at
        // startup the local and the shared state are identical by construction,
        // so without this the sequencer stays silent until the first keypress.
        self.state = self.read_shared_state().await;
        self.send_sequence().await;

        loop {
            let r_state = self.read_shared_state().await;
            if r_state != self.state {
                debug!(
                    "Euclidean sequencer {:?} new state: {:?}",
                    self.sequencer_slot, r_state
                );
                self.state = r_state;
                self.send_sequence().await;
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(steps: usize, pulses: usize, phase: usize) -> String {
        let sequencer = EuclideanSequencer {
            sequencer_slot: SequencerSlot::Left,
            state: EuclideanSequencerState {
                steps,
                pulses,
                phase,
                pitch: 60,
            },
            shared_state: Arc::new(RwLock::new(SharedState::default())),
            tx_sequence: mpsc::channel(1).0,
        };

        sequencer
            .generate_sequence()
            .notes
            .iter()
            .map(|n| if n.is_rest() { '.' } else { 'x' })
            .collect()
    }

    /// Note these are the Bresenham approximation the sequencer actually uses,
    /// not canonical Euclidean rhythms: E(3,8) is conventionally `x..x..x.`,
    /// whereas `(i * steps) / pulses` yields onsets at {0, 2, 5}.
    #[test]
    fn test_euclidean_patterns() {
        assert_eq!(pattern(8, 0, 0), "........");
        assert_eq!(pattern(8, 8, 0), "xxxxxxxx");
        assert_eq!(pattern(8, 4, 0), "x.x.x.x.");
        assert_eq!(pattern(8, 3, 0), "x.x..x..");
        assert_eq!(pattern(16, 4, 0), "x...x...x...x...");
    }

    #[test]
    fn test_phase_rotates_the_pattern() {
        assert_eq!(pattern(8, 3, 1), ".x.x..x.");
        assert_eq!(pattern(8, 3, 2), "..x.x..x");
    }

    #[test]
    fn test_pulse_count_is_honoured() {
        for steps in 1..=16 {
            for pulses in 0..=steps {
                let struck = pattern(steps, pulses, 0).matches('x').count();
                assert_eq!(
                    struck, pulses,
                    "steps={steps} pulses={pulses} produced {struck} onsets"
                );
            }
        }
    }
}

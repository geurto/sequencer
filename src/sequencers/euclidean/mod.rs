pub mod state;

use std::sync::Arc;

use crate::sequencers::euclidean::state::EuclideanSequencerState;
use crate::sequencers::{Note, NoteDuration, Sequence, Sequencer};

use crate::playback::state::{SequencerSlot, SharedState};
use log::{debug, error};
use tokio::sync::{mpsc, RwLock};

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
}

impl Sequencer for EuclideanSequencer {
    async fn generate_sequence(&self) -> Sequence {
        if self.state.pulses == 0 {
            // Handle zero pulses case
            let note = Note::new(0, 0, NoteDuration::Sixteenth);
            return Sequence {
                notes: vec![note; self.state.steps],
            };
        }

        let mut sequence = Sequence::empty();

        // Bresenham line algorithm cus it looks easier
        let beat_locations = (0..self.state.pulses)
            .map(|i| (i * self.state.steps) / self.state.pulses)
            .collect::<Vec<_>>();

        for i in 0..self.state.steps {
            let note = if beat_locations.contains(&(i % self.state.steps)) {
                Note::new(self.state.pitch, 100, NoteDuration::Sixteenth)
            } else {
                Note::new(0, 0, NoteDuration::Sixteenth)
            };
            sequence.notes.push(note);
        }
        sequence
    }

    async fn run(&mut self) {
        loop {
            let r_state = match self.sequencer_slot {
                SequencerSlot::Left => {
                    self.shared_state.read().await.left_sequencer
                }
                SequencerSlot::Right => {
                    self.shared_state.read().await.right_sequencer
                }
            };
            if r_state != self.state {
                debug!(
                    "Euclidean sequencer {:?} new state: {:?}",
                    self.sequencer_slot, r_state
                );
                self.state = r_state;

                let sequence = self.generate_sequence().await;
                {
                    match self.sequencer_slot {
                        SequencerSlot::Left => {
                            debug!(
                                "Sending {:?} sequence to mixer",
                                self.sequencer_slot
                            );
                            if let Err(e) = self
                                .tx_sequence
                                .send((Some(sequence), None))
                                .await
                            {
                                error!("Error sending left Sequence: {e}");
                            }
                        }
                        SequencerSlot::Right => {
                            if let Err(e) = self
                                .tx_sequence
                                .send((None, Some(sequence)))
                                .await
                            {
                                error!("Error sending right Sequence: {e}");
                            }
                        }
                    };
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }
}

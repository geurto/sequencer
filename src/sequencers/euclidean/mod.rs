pub mod gui;
pub mod state;

use crate::note::{Note, NoteDuration, Sequence};
use crate::sequencers::common::Sequencer;
use crate::sequencers::euclidean::state::EuclideanSequencerState;

use crate::state::{SequencerSlot, SharedState};
use anyhow::Result;
use log::info;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::RwLock;

pub struct EuclideanSequencer {
    sequencer_slot: SequencerSlot,
    tx_sequence: mpsc::Sender<(Option<Sequence>, Option<Sequence>)>,
    cached_state: EuclideanSequencerState,
    shared_state: Arc<RwLock<SharedState>>,
}

impl EuclideanSequencer {
    pub fn new(
        sequencer_slot: SequencerSlot,
        tx_sequence: mpsc::Sender<(Option<Sequence>, Option<Sequence>)>,
        shared_state: Arc<RwLock<SharedState>>,
    ) -> Self {
        EuclideanSequencer {
            sequencer_slot,
            tx_sequence,
            cached_state: EuclideanSequencerState::new(),
            shared_state,
        }
    }
}

impl Sequencer for EuclideanSequencer {
    async fn generate_sequence(&self) -> Sequence {
        let bpm = self.shared_state.read().await.bpm;

        if self.cached_state.pulses == 0 {
            // Handle zero pulses case
            let note = Note::new(0, 0, NoteDuration::Sixteenth, bpm);
            return Sequence {
                notes: vec![note; 16],
            };
        }

        let mut sequence = Sequence::empty();

        // Bresenham line algorithm cus it looks easier
        let beat_locations = (0..self.cached_state.pulses)
            .map(|i| (i * self.cached_state.steps) / self.cached_state.pulses)
            .collect::<Vec<_>>();

        for i in 0..self.cached_state.steps {
            let note = if beat_locations.contains(&(i % self.cached_state.steps)) {
                Note::new(self.cached_state.pitch, 100, NoteDuration::Sixteenth, bpm)
            } else {
                Note::new(0, 0, NoteDuration::Sixteenth, bpm)
            };
            sequence.notes.push(note);
        }
        info!("Generated sequence {:?}", sequence);
        sequence
    }

    async fn run(&mut self) -> Result<()> {
        let mut previous_state = EuclideanSequencerState::new();

        loop {
            let state = match self.sequencer_slot {
                SequencerSlot::Left => self.shared_state.read().await.left_state.clone(),
                SequencerSlot::Right => self.shared_state.read().await.right_state.clone(),
            };

            if state != previous_state {
                info!(
                    "Euclidean sequencer {:?} new config: {:?}",
                    self.sequencer_slot, state
                );
                self.cached_state = state.clone();
                let sequence = self.generate_sequence().await;
                {
                    match self.sequencer_slot {
                        SequencerSlot::Left => {
                            info!(
                                "Sending sequence {:?} to slot {:?}",
                                sequence, self.sequencer_slot
                            );
                            self.tx_sequence.send((Some(sequence), None)).await?;
                        }
                        SequencerSlot::Right => {
                            self.tx_sequence.send((None, Some(sequence))).await?
                        }
                    };
                }
                previous_state = state;
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }
}

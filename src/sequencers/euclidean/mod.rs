pub mod state;

use std::sync::Arc;
use std::time::Duration;

use crate::sequencers::euclidean::state::EuclideanSequencerState;

use crate::playback::state::{SequencerSlot, SharedState};
use log::{debug, error};
use seq_core::{Pattern, euclid};
use tokio::sync::{RwLock, mpsc};

/// Velocity of a struck pulse. The mixer scales this by the crossfade ratio.
const PULSE_VELOCITY: u8 = 100;

#[derive(Debug)]
pub struct EuclideanSequencer {
    sequencer_slot: SequencerSlot,
    state: EuclideanSequencerState,
    shared_state: Arc<RwLock<SharedState>>,
    tx_pattern: mpsc::Sender<(Option<Pattern>, Option<Pattern>)>,
}

impl EuclideanSequencer {
    pub fn new(
        sequencer_slot: SequencerSlot,
        shared_state: Arc<RwLock<SharedState>>,
        tx_pattern: mpsc::Sender<(Option<Pattern>, Option<Pattern>)>,
    ) -> Self {
        EuclideanSequencer {
            sequencer_slot,
            state: EuclideanSequencerState::new(),
            shared_state,
            tx_pattern,
        }
    }

    /// The pattern for the current parameters. Generation itself is the pure
    /// [`seq_core::euclid::pattern`]; this only forwards the knob state.
    #[must_use]
    pub fn generate_pattern(&self) -> Pattern {
        euclid::pattern(
            self.state.steps,
            self.state.pulses,
            self.state.phase,
            self.state.pitch,
            PULSE_VELOCITY,
        )
    }

    async fn read_shared_state(&self) -> EuclideanSequencerState {
        let shared = self.shared_state.read().await;
        match self.sequencer_slot {
            SequencerSlot::Left => shared.left_sequencer,
            SequencerSlot::Right => shared.right_sequencer,
        }
    }

    async fn send_pattern(&self) {
        debug!("Sending {:?} pattern to mixer", self.sequencer_slot);
        let pattern = self.generate_pattern();
        let payload = match self.sequencer_slot {
            SequencerSlot::Left => (Some(pattern), None),
            SequencerSlot::Right => (None, Some(pattern)),
        };
        if let Err(e) = self.tx_pattern.send(payload).await {
            error!("Error sending {:?} Pattern: {e}", self.sequencer_slot);
        }
    }

    pub async fn run(&mut self) {
        // Emit once up front. The loop below only reacts to *changes*, and at
        // startup the local and the shared state are identical by construction,
        // so without this the sequencer stays silent until the first keypress.
        self.state = self.read_shared_state().await;
        self.send_pattern().await;

        loop {
            let r_state = self.read_shared_state().await;
            if r_state != self.state {
                debug!(
                    "Euclidean sequencer {:?} new state: {:?}",
                    self.sequencer_slot, r_state
                );
                self.state = r_state;
                self.send_pattern().await;
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

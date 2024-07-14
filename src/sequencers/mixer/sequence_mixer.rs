use log::debug;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::sync::mpsc::Receiver;
use crate::note::Sequence;
use crate::sequencers::mixer::config::MixerConfig;
use crate::state::SharedState;

pub struct Mixer {
    update_rx: Receiver<()>,
    shared_state: Arc<Mutex<SharedState>>,
}

impl Mixer {
    pub fn new(update_rx: Receiver<()>, shared_state: Arc<Mutex<SharedState>>) -> Self {
        Mixer { update_rx, shared_state }
    }

    pub async fn run(&mut self) {
        loop {
            if let Some(()) = self.update_rx.recv().await {
                debug!("Mixer received update request");
                self.mix().await;
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }

    pub async fn mix(&mut self) {
        // TODO: implement normally
        let mut state = self.shared_state.lock().await;

        let mut mixed_sequence = Sequence::empty();
        let num_notes = state.mixer_config.sequence_a.notes.len();

        for i in 0..num_notes {
            let note_a = state.mixer_config.sequence_a.notes.get(i);

            if let Some(note_a) = note_a {
                let note = note_a.clone();
                mixed_sequence.notes.push(note);
            }
        }
        state.mixer_config.mixed_sequence = mixed_sequence;
    }
}
use log::debug;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::sync::mpsc::Receiver;
use crate::note::Sequence;
use crate::sequencers::mixer::config::MixerConfig;
use crate::state::SharedState;

pub struct Mixer {
    config: MixerConfig,
    config_rx: Receiver<MixerConfig>,
}

impl Mixer {
    pub fn new(config_rx: Receiver<MixerConfig>) -> Self {
        let config = MixerConfig::new();
        Mixer { config, config_rx }
    }

    pub async fn run(&mut self) {
        loop {
            if let Some(config) = self.config_rx.recv().await {
                debug!("Mixer received config: {:?}", config);
                self.config = config;
                self.mix();
            }
        }
    }

pub fn mix(&mut self) {
    // TODO: implement normally
    let mut mixed_sequence = Sequence::empty();
    let num_notes = self.config.sequence_a.notes.len();

    for i in 0..num_notes {
        let note_a = self.config.sequence_a.notes.get(i);

        if let Some(note_a) = note_a {
            let note = note_a.clone();
            mixed_sequence.notes.push(note);
        }
    }
    self.config.mixed_sequence = mixed_sequence;
}
}
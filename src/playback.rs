use anyhow::Error;
use log::{debug, info};
use tokio::sync::{mpsc, Mutex};
use std::sync::Arc;
use tokio::time::{self, Duration};

use crate::state::*;
use crate::midi::MidiHandler;
use crate::input::Input;
use crate::sequencers::euclidean::config::EuclideanSequencerConfig;
use crate::sequencers::euclidean::input::EuclideanSequencerInput;
use crate::sequencers::markov::config::MarkovSequencerConfig;
use crate::sequencers::mixer::sequence_mixer::Mixer;

pub async fn start_playback_loop(
    mut midi_handler: MidiHandler,
    shared_state: Arc<Mutex<SharedState>>,
) -> Result<(), Error> {
    info!("Starting playback loop");

     tokio::spawn(async move {
        loop {
            let state = shared_state.lock().await;
            if state.playing {
                let sequence = state.mixer_config.mixed_sequence.clone();
                debug!("Playing {:?}", sequence);
                 for i in 0..sequence.notes.len() {
                    let pitch = sequence.notes[i].pitch;
                    let duration = Duration::from_millis(sequence.notes[i].duration as u64);
                    let velocity = sequence.notes[i].velocity;
                    midi_handler.send_note_on(pitch, velocity).expect("Failed to send NOTE ON");
                    time::sleep(duration).await;
                    midi_handler.send_note_off(pitch).expect("Failed to send NOTE OFF");
                }
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    Ok(())
}

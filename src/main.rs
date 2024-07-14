mod playback;
mod sequencers;
mod state;
mod midi;
mod input;
mod note;

use anyhow::Error;
use ctrlc;
use env_logger::Builder;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::{mpsc, Mutex};

use crate::state::{SequencerChannels, SharedState};
use crate::midi::MidiHandler;
use crate::sequencers::euclidean::euclidean_sequencer::EuclideanSequencer;
use crate::sequencers::markov::markov_sequencer::MarkovSequencer;
use crate::sequencers::mixer::sequence_mixer::Mixer;
use crate::sequencers::traits::Sequencer;

#[tokio::main]
async fn main() -> Result<(), Error> {
    Builder::new().filter(None, log::LevelFilter::Debug).init();

    // tokio channels
    let (tx_input, rx_input) = mpsc::channel(1);
    let (tx_config_euclidean, rx_config_euclidean) = mpsc::channel(1);
    let (tx_config_markov, rx_config_markov) = mpsc::channel(1);
    let (tx_update_mixer, rx_update_mixer) = mpsc::channel(1);
    let (tx_shutdown, mut rx_shutdown) = mpsc::channel(1);

    let sequencer_channels = SequencerChannels {
        euclidean_tx: tx_config_euclidean,
        markov_tx: tx_config_markov,
        mixer_tx: tx_update_mixer.clone(),
    };

    let shared_state = Arc::new(Mutex::new(SharedState::new(120.0)));

    // MIDI input and output
    let midi_handler = Arc::new(Mutex::new(MidiHandler::new()?));
    midi_handler.lock().await.setup_midi_input(shared_state.clone()).await?;

    let tx_ctrlc = tx_input.clone();
    let tx_shutdown_ctrlc = tx_shutdown.clone();
    ctrlc::set_handler(move || {
        let tx_ctrlc = tx_ctrlc.clone();
        let tx_shutdown_ctrlc = tx_shutdown_ctrlc.clone();
        tokio::spawn(async move {
            tx_shutdown_ctrlc.send(()).await.unwrap();
        });
    })?;

    // Sequencers and mixer
    let mut sequencer_a  = EuclideanSequencer::new(
        rx_config_euclidean,
        tx_update_mixer.clone(),
        shared_state.clone());
    sequencer_a.generate_sequence().await;
    tokio::spawn(async move {
        sequencer_a.run(0).await.unwrap();
    });

    let mut sequencer_b = MarkovSequencer::new(
        rx_config_markov,
        tx_update_mixer.clone(),
        shared_state.clone());
    sequencer_b.generate_sequence().await;
    tokio::spawn(async move {
        sequencer_b.run(1).await.unwrap();
    });

    let mut sequence_mixer = Mixer::new(rx_update_mixer, shared_state.clone());
    sequence_mixer.mix().await;
    tokio::spawn(async move {
        sequence_mixer.run().await;
    });

    // Input handling
    input::spawn_input_handler(tx_input.clone());
    let shared_state_input = shared_state.clone();
    tokio::spawn(async move {
        input::process_input(rx_input, shared_state_input, sequencer_channels).await;
    });

    // Playback
    let shared_state_playback = shared_state.clone();
    let midi_handler_clone = midi_handler.clone();
    tokio::spawn(async move {
        playback::play(midi_handler_clone, shared_state_playback).await.unwrap();
    });

    tokio::select! {
        _ = rx_shutdown.recv() => {
            println!("Shutdown signal received, exiting...");
        }
        _ = signal::ctrl_c() => {
            println!("Ctrl+C received, exiting...");
        }
    }

    Ok(())
}
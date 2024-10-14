mod playback;
mod sequencers;
mod state;
mod midi;
mod input;
mod note;

use anyhow::Error;
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
    let (tx_config_a, rx_config_a) = mpsc::channel(1);
    let (tx_config_b, rx_config_b) = mpsc::channel(1);
    let (tx_update_mixer, rx_update_mixer) = mpsc::channel(1);

    let sequencer_channels = SequencerChannels {
        a_tx: tx_config_a,
        b_tx: tx_config_b,
        mixer_tx: tx_update_mixer.clone(),
    };

    let shared_state = Arc::new(Mutex::new(SharedState::new(120.0)));

    // MIDI input and output
    let midi_handler = Arc::new(Mutex::new(MidiHandler::new()?));
    midi_handler.lock().await.setup_midi_input(shared_state.clone()).await?;

    // Sequencers and mixer
    let mut sequencer_a  = EuclideanSequencer::new(
        rx_config_a,
        tx_update_mixer.clone(),
        shared_state.clone());
    sequencer_a.generate_sequence().await;
    tokio::spawn(async move {
        sequencer_a.run(0).await.unwrap();
    });

    let mut sequencer_b = MarkovSequencer::new(
        rx_config_b,
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

    // Shutdown
    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to install Ctrl+C handler");
        println!("Ctrl+C received, exiting...");
        std::process::exit(0);
    });

    Ok(())
}
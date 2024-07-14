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

    let mut midi_handler = MidiHandler::new()?;
    let (tx_input, rx_input) = mpsc::channel(1);
    let (tx_config_euclidean, rx_config_euclidean) = mpsc::channel(1);
    let (tx_config_markov, rx_config_markov) = mpsc::channel(1);
    let (tx_config_mixer, rx_config_mixer) = mpsc::channel(1);
    let (tx_shutdown, mut rx_shutdown) = mpsc::channel(1);

    let sequencer_channels = SequencerChannels {
        euclidean_tx: tx_config_euclidean,
        markov_tx: tx_config_markov,
        mixer_tx: tx_config_mixer,
    };

    let shared_state = Arc::new(Mutex::new(SharedState::new(120.0)));
    midi_handler.setup_midi_input(shared_state.clone()).await?;

    playback::start_playback_loop(midi_handler, shared_state.clone()).await?;

    let mut sequencer_a = MarkovSequencer::new(rx_config_markov, shared_state.clone());
    let mut sequencer_b = EuclideanSequencer::new(rx_config_euclidean, shared_state.clone());
    let mut sequence_mixer = Mixer::new(rx_config_mixer);

    input::spawn_input_handler(tx_input.clone());
    tokio::spawn(async move {
        input::process_input(rx_input, shared_state.clone(), sequencer_channels).await;
    });

    let tx_ctrlc = tx_input.clone();
    let tx_shutdown_ctrlc = tx_shutdown.clone();
    ctrlc::set_handler(move || {
        let tx_ctrlc = tx_ctrlc.clone();
        let tx_shutdown_ctrlc = tx_shutdown_ctrlc.clone();
        tokio::spawn(async move {
            tx_shutdown_ctrlc.send(()).await.unwrap();
        });
    })?;

    tokio::spawn(async move {
        sequencer_a.run(0).await.unwrap();
    });
    tokio::spawn(async move {
        sequencer_b.run(1).await.unwrap();
    });
    tokio::spawn(async move {
        sequence_mixer.run().await;
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
mod playback;
mod sequencers;
mod common;
mod midi;
mod input;

use ctrlc;
use env_logger::Builder;
use std::{error::Error, sync::Arc};
use tokio::sync::{mpsc, Mutex};

use crate::common::{Input, SequencerChannels, SharedState};
use crate::midi::MidiHandler;
use crate::sequencers::markov::MarkovSequencer;
use crate::sequencers::euclidean::EuclideanSequencer;
use crate::sequencers::Sequencer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    Builder::new().filter(None, log::LevelFilter::Debug).init();

    let mut midi_handler = MidiHandler::new()?;
    let (tx_input, rx_input) = mpsc::channel(1);
    let (tx_config_euclidean, rx_config_euclidean) = mpsc::channel(1);
    let (tx_config_markov, rx_config_markov) = mpsc::channel(1);
    let sequencer_channels = SequencerChannels {
        euclidean_tx: tx_config_euclidean,
        markov_tx: tx_config_markov,
    };

    let shared_state = Arc::new(Mutex::new(SharedState::new(120.0)));
    midi_handler.setup_midi_input(shared_state.clone()).await?;

    playback::start_playback_loop(midi_handler, tx_input.clone(), rx_input, shared_state.clone(), sequencer_channels).await?;

    let markov_sequencer = MarkovSequencer::new(rx_config_markov, shared_state.clone());
    let euclidean_sequencer = EuclideanSequencer::new(rx_config_euclidean, shared_state.clone());

    input::spawn_input_handler(tx_input.clone());

    let tx_ctrlc = tx_input.clone();
    ctrlc::set_handler(move || {
        let tx_ctrlc = tx_ctrlc.clone();
        tokio::spawn(async move {
            tx_ctrlc.send(Input::Shutdown).await.unwrap();
        });
    }).expect("Error setting Ctrl-C handler");

    loop {
        let new_sequence = euclidean_sequencer.generate_sequence(16).await;
        tx_input.send(Input::Sequence(new_sequence)).await.unwrap();
    }
}
mod playback;
mod sequencers;
mod common;
mod midi;
mod input;

use tokio::sync::mpsc;
use std::{error::Error, time::Duration};
use crate::common::{Input, Sequence};
use crate::midi::MidiHandler;
use crate::sequencers::markov::MarkovSequencer;
use crate::sequencers::euclidean::EuclideanSequencer;
use crate::sequencers::Sequencer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut midi_handler = MidiHandler::new()?;
    let (tx, rx) = mpsc::channel(32);

    playback::start_playback_loop(midi_handler, tx.clone(), rx).await?;

    // let sequencer = MarkovSequencer::new();
    let sequencer = EuclideanSequencer::new(16, 7, 60);

    let tx_clone = tx.clone();
    tokio::spawn(async move {
        input::handle_user_input(tx_clone).await;
    });

    loop {
        let new_sequence = sequencer.generate_sequence(16);
        tx.send(Input::Sequence(new_sequence)).await.unwrap();
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
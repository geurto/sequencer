use anyhow::Result;
use env_logger::Builder;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::{mpsc, Mutex};

use sequencer::{
    input::{process_input, spawn_input_handler},
    playback::play,
    EuclideanSequencer, Gui, MidiHandler, Mixer, Sequencer, SharedState,
};

#[tokio::main]
async fn main() -> Result<()> {
    Builder::new().filter(None, log::LevelFilter::Info).init();

    // tokio channels
    let (tx_input, rx_input) = mpsc::channel(1);
    let (tx_config_a, rx_config_a) = mpsc::channel(1);
    let (tx_config_b, rx_config_b) = mpsc::channel(1);
    let (tx_update_mixer, rx_update_mixer) = mpsc::channel(1);
    let (tx_gui, rx_gui) = mpsc::channel(1);

    let shared_state = Arc::new(Mutex::new(SharedState::new(120.0)));

    // MIDI input and output
    let midi_handler = Arc::new(Mutex::new(MidiHandler::new()?));
    midi_handler
        .lock()
        .await
        .setup_midi_input(shared_state.clone())
        .await?;

    let mut handles = vec![];

    // Sequencers and mixer
    let mut sequencer_a = EuclideanSequencer::new(
        rx_config_a,
        tx_gui,
        tx_update_mixer.clone(),
        shared_state.clone(),
    );
    sequencer_a.generate_sequence().await;
    handles.push(tokio::spawn(async move {
        sequencer_a.run(0).await.unwrap();
    }));

    // both Euclidean for now to keep it simple
    let mut sequencer_b = EuclideanSequencer::new(
        rx_config_b,
        tx_gui,
        tx_update_mixer.clone(),
        shared_state.clone(),
    );
    sequencer_b.generate_sequence().await;
    handles.push(tokio::spawn(async move {
        sequencer_b.run(1).await.unwrap();
    }));

    let mut sequence_mixer = Mixer::new(rx_update_mixer, shared_state.clone());
    sequence_mixer.mix().await;
    handles.push(tokio::spawn(async move {
        sequence_mixer.run().await;
    }));

    // Input handling
    spawn_input_handler(tx_input.clone());
    let shared_state_input = shared_state.clone();
    handles.push(tokio::spawn(async move {
        process_input(rx_input, shared_state_input).await;
    }));

    // Playback
    let shared_state_playback = shared_state.clone();
    let midi_handler_clone = midi_handler.clone();
    handles.push(tokio::spawn(async move {
        play(midi_handler_clone, shared_state_playback)
            .await
            .unwrap();
    }));

    // Shutdown
    let _ctrl_c_handle = tokio::spawn(async move {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        println!("Ctrl+C received, exiting...");
        std::process::exit(0);
    });

    // GUI
    Gui::run(rx_gui)?;
    Ok(())
}

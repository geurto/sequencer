use anyhow::Result;
use device_query::Keycode;
use env_logger::Builder;
use std::{
    collections::HashSet,
    sync::{Arc, Mutex as SyncMutex},
};
use tokio::signal;
use tokio::sync::{mpsc, RwLock};

use sequencer::{
    gui::Message,
    run_input_handler,
    sequencers::{euclidean::gui::Gui as EuclideanGui, mixer::gui::Gui as MixerGui},
    start_polling,
    state::SequencerSlot,
    EuclideanSequencer, Gui, MidiHandler, Mixer, PlaybackHandler, Sequence, Sequencer, SharedState,
};

#[tokio::main]
async fn main() -> Result<()> {
    Builder::new().filter(None, log::LevelFilter::Info).init();

    // key input handling
    let (tx_keys, rx_keys) = mpsc::channel::<HashSet<Keycode>>(100);

    // separate sequences from left/right - (Option<Sequence>, Option<Sequence>)
    let (tx_sequence, rx_sequence) = mpsc::channel::<(Option<Sequence>, Option<Sequence>)>(1);

    // final sequence for playback - Sequence
    let (tx_mixed_sequence, rx_mixed_sequence) = mpsc::channel::<Sequence>(1);

    let shared_state: Arc<RwLock<SharedState>> = Arc::new(RwLock::new(SharedState::new(120.)));

    let tx_gui: Arc<SyncMutex<Option<iced::futures::channel::mpsc::Sender<Message>>>> =
        Arc::new(SyncMutex::new(None));

    // Sequencers and mixer
    let mut sequencer_a = EuclideanSequencer::new(
        SequencerSlot::Left,
        tx_sequence.clone(),
        shared_state.clone(),
    );
    sequencer_a.generate_sequence().await;
    tokio::spawn(async move {
        sequencer_a.run().await.unwrap();
    });

    // both Euclidean for now to keep it simple
    let mut sequencer_b = EuclideanSequencer::new(
        SequencerSlot::Right,
        tx_sequence.clone(),
        shared_state.clone(),
    );
    sequencer_b.generate_sequence().await;
    tokio::spawn(async move { sequencer_b.run().await });

    let mut sequence_mixer = Mixer::new(shared_state.clone(), tx_mixed_sequence, rx_sequence);
    sequence_mixer.mix().await;
    tokio::spawn(async move { sequence_mixer.run().await });

    // Input handling
    start_polling(tx_keys);
    let shared_state_input = shared_state.clone();
    let tx_gui_input = tx_gui.clone();
    tokio::spawn(async move { run_input_handler(rx_keys, tx_gui_input, shared_state_input).await });

    // Playback
    let midi_handler = MidiHandler::new()?;
    let tx_gui_playback = tx_gui.clone();
    let mut playback_handler = PlaybackHandler::new(
        midi_handler,
        rx_mixed_sequence,
        tx_gui_playback,
        shared_state.clone(),
    );
    tokio::spawn(async move { playback_handler.run().await });

    // GUI
    let gui_sequencer_left = EuclideanGui::new(SequencerSlot::Left);
    let gui_sequencer_right = EuclideanGui::new(SequencerSlot::Right);
    let gui_mixer = MixerGui::new();

    Gui::run(
        tx_gui.clone(),
        gui_sequencer_left,
        gui_sequencer_right,
        gui_mixer,
    )?;

    // Shutdown
    let _ctrl_c_handle = tokio::spawn(async move {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        println!("Ctrl+C received, exiting...");
        std::process::exit(0);
    });

    Ok(())
}

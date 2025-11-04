use anyhow::Result;
use env_logger::Builder;
use std::{
    sync::{mpsc::channel as sync_channel, Arc, Mutex as SyncMutex},
    thread,
};
use tokio::signal;
use tokio::sync::{mpsc, RwLock};

use sequencer::{
    gui::Message, midi_utils, playback::state::PolyphonicSequence,
    sequencers::euclidean::gui::Gui as EuclideanGui, state::SequencerSlot,
    EuclideanSequencer, Gui, MidiCommand, Mixer, PlaybackEngine,
    PlaybackHandler, Sequence, Sequencer, SharedState,
};

#[tokio::main]
async fn main() -> Result<()> {
    Builder::new().filter(None, log::LevelFilter::Info).init();

    // sequences FROM sequencers TO mixer
    let (tx_sequence, rx_sequence) =
        mpsc::channel::<(Option<Sequence>, Option<Sequence>)>(1);

    // mixed sequence FROM mixer TO playback_handler
    let (tx_mixed_sequence, rx_mixed_sequence) =
        mpsc::channel::<PolyphonicSequence>(1);

    // MIDI messages, either GUI or playing a note
    let (tx_midi, rx_midi) = mpsc::channel::<MidiCommand>(1);

    // synchronous playback commands & status
    let (tx_playback_cmd, rx_playback_cmd) = sync_channel();
    let (tx_playback_status, rx_playback_status) = sync_channel();

    let shared_state: Arc<RwLock<SharedState>> =
        Arc::new(RwLock::new(SharedState::new(120.)));

    let tx_gui: Arc<
        SyncMutex<Option<iced::futures::channel::mpsc::Sender<Message>>>,
    > = Arc::new(SyncMutex::new(None));

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

    let mut sequence_mixer =
        Mixer::new(shared_state.clone(), tx_mixed_sequence, rx_sequence);
    sequence_mixer.mix().await;
    tokio::spawn(async move { sequence_mixer.run().await });

    // Playback
    let midi_ports = midi_utils::list_ports()?;
    let midi_conn = midi_utils::create_connection(midi_ports[0].clone())?;

    // Link between async GUI and sync playback engine
    let shared_state_input = shared_state.clone();
    let tx_gui_playback = tx_gui.clone();
    let mut playback_handler = PlaybackHandler::new(
        rx_midi,
        rx_mixed_sequence,
        rx_playback_status,
        tx_playback_cmd,
        tx_gui_playback,
        shared_state_input,
    );
    tokio::spawn(async move { playback_handler.run().await });

    // Synchronous playback engine that handles MIDI control
    let playback_engine =
        PlaybackEngine::new(rx_playback_cmd, tx_playback_status, midi_conn);
    thread::spawn(move || {
        playback_engine.run();
    });

    // GUI
    let gui_sequencer_left = EuclideanGui::new(SequencerSlot::Left);
    let gui_sequencer_right = EuclideanGui::new(SequencerSlot::Right);

    Gui::run(
        tx_gui.clone(),
        tx_midi,
        gui_sequencer_left,
        gui_sequencer_right,
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

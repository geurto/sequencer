use anyhow::{anyhow, Result};
use env_logger::Builder;
use log::{error, info};
use std::{
    sync::{mpsc::channel as sync_channel, Arc, Mutex as SyncMutex},
    thread,
};
use tokio::sync::mpsc;
use tokio::{signal, sync::RwLock};

use sequencer::{
    gui::{sequencers::euclidean::Gui as EuclideanGui, state::GuiMessage},
    midi_utils,
    playback::state::{PolyphonicSequence, SequencerSlot, SharedState},
    EuclideanSequencer, Gui, MidiCommand, Mixer, PlaybackEngine,
    PlaybackHandler, PlaybackStatus, Sequence, Sequencer,
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
    let (tx_playback_status, rx_playback_status) =
        mpsc::unbounded_channel::<PlaybackStatus>();

    // state updates to GUI
    let tx_gui: Arc<
        SyncMutex<Option<iced::futures::channel::mpsc::Sender<GuiMessage>>>,
    > = Arc::new(SyncMutex::new(None));

    // shared state which is read by multiple structs
    let shared_state: Arc<RwLock<SharedState>> =
        Arc::new(RwLock::new(SharedState::new(120.)));

    // Sequencers and mixer. Each sequencer emits its opening sequence as soon
    // as it starts running, so there is nothing to prime here.
    let mut sequencer_a = EuclideanSequencer::new(
        SequencerSlot::Left,
        shared_state.clone(),
        tx_sequence.clone(),
    );
    tokio::spawn(async move {
        sequencer_a.run().await;
    });

    // both Euclidean for now to keep it simple
    let mut sequencer_b = EuclideanSequencer::new(
        SequencerSlot::Right,
        shared_state.clone(),
        tx_sequence.clone(),
    );
    tokio::spawn(async move { sequencer_b.run().await });

    let mut sequence_mixer =
        Mixer::new(shared_state.clone(), rx_sequence, tx_mixed_sequence);
    tokio::spawn(async move { sequence_mixer.run().await });

    // Playback
    let midi_ports = midi_utils::list_ports()?;
    let out_port = midi_ports.first().ok_or_else(|| {
        anyhow!(
            "No MIDI output ports found. Start a synthesiser (e.g. FluidSynth) \
             or connect a MIDI device, then run the sequencer again."
        )
    })?;
    info!("Connecting to MIDI output port {out_port}");
    let midi_conn = midi_utils::create_connection(out_port)?;

    // Link between async GUI and sync playback engine
    let tx_gui_playback = tx_gui.clone();
    let mut playback_handler = PlaybackHandler::new(
        shared_state.clone(),
        rx_midi,
        rx_mixed_sequence,
        tx_playback_cmd,
        rx_playback_status,
        tx_gui_playback,
    );
    tokio::spawn(async move { playback_handler.run().await });

    // Synchronous playback engine that handles MIDI control
    let playback_engine = PlaybackEngine::new(
        rx_playback_cmd,
        tx_playback_status,
        Box::new(midi_conn),
    );
    thread::spawn(move || {
        playback_engine.run();
    });

    // Shutdown. This has to be installed *before* the GUI takes over the
    // calling thread: spawning it afterwards means it only starts once the
    // window has already closed and main is about to return anyway.
    tokio::spawn(async move {
        if let Err(e) = signal::ctrl_c().await {
            error!("Failed to install Ctrl+C handler: {e}");
            return;
        }
        info!("Ctrl+C received, exiting...");
        std::process::exit(0);
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

    Ok(())
}

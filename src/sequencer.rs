//! The one task that owns the sequencer's state.
//!
//! Everything that used to poll — two generator tasks, the mixer task and the
//! playback handler, each waking every 10 ms to diff a lock — collapses into
//! this. State has a single owner, so a change is *known* rather than
//! discovered: a [`ControlEvent`] arrives, the state applies it, the affected
//! work is redone, and a fresh [`UiSnapshot`] is published. Nothing spins.

use log::{error, info, warn};
use seq_ui::{ControlEvent, UiSnapshot};
use std::sync::mpsc::Sender as SyncSender;
use tokio::sync::{mpsc, watch};

use crate::playback::midi::{MidiCommand, MidirSink, midi_utils};
use crate::playback::state::PlaybackCommand;
use crate::state::{SequencerState, bpm_to_milli};

/// What woke the task up.
enum Wake {
    Control(ControlEvent),
    Midi(MidiCommand),
    StepChanged,
    /// A secondary input closed; it has been retired and there is no work.
    Nothing,
    Shutdown,
}

pub struct Sequencer {
    state: SequencerState,

    rx_control: mpsc::Receiver<ControlEvent>,
    rx_midi: mpsc::Receiver<MidiCommand>,
    rx_step: watch::Receiver<usize>,
    tx_engine: SyncSender<PlaybackCommand>,
    tx_snapshot: watch::Sender<UiSnapshot>,
}

impl std::fmt::Debug for Sequencer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sequencer")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl Sequencer {
    #[must_use]
    pub fn new(
        state: SequencerState,
        rx_control: mpsc::Receiver<ControlEvent>,
        rx_midi: mpsc::Receiver<MidiCommand>,
        rx_step: watch::Receiver<usize>,
        tx_engine: SyncSender<PlaybackCommand>,
        tx_snapshot: watch::Sender<UiSnapshot>,
    ) -> Self {
        Self {
            state,
            rx_control,
            rx_midi,
            rx_step,
            tx_engine,
            tx_snapshot,
        }
    }

    pub async fn run(&mut self) {
        // The engine boots with its own defaults; make this state
        // authoritative, and emit the opening pattern, before anything plays.
        self.send_engine(PlaybackCommand::SetBPM(self.state.bpm));
        self.send_engine(PlaybackCommand::SetMidiChannel(
            self.state.midi_channel,
        ));
        self.regenerate();
        self.publish();

        // A closed channel is *always* ready, so a dead input would win the
        // select forever and starve the live ones. Retire each secondary input
        // as it closes; the control channel closing is what ends the task.
        let mut midi_open = true;
        let mut steps_open = true;

        loop {
            // Each branch only names the value it woke on; the work happens
            // below, where `&mut self` is free again.
            let wake = tokio::select! {
                control = self.rx_control.recv() => {
                    control.map_or(Wake::Shutdown, Wake::Control)
                }
                command = self.rx_midi.recv(), if midi_open => {
                    if let Some(command) = command {
                        Wake::Midi(command)
                    } else {
                        midi_open = false;
                        Wake::Nothing
                    }
                }
                changed = self.rx_step.changed(), if steps_open => {
                    if changed.is_ok() {
                        Wake::StepChanged
                    } else {
                        steps_open = false;
                        Wake::Nothing
                    }
                }
            };

            match wake {
                Wake::Control(event) => self.handle_control(event),
                Wake::Midi(command) => self.handle_midi_command(command),
                Wake::StepChanged => {
                    self.state.step_index = *self.rx_step.borrow_and_update();
                    self.publish();
                }
                Wake::Nothing => {}
                Wake::Shutdown => break,
            }
        }

        info!("Sequencer task shutting down");
    }

    fn handle_control(&mut self, event: ControlEvent) {
        self.state.apply(event);

        // Only redo the work the change actually invalidated.
        match event {
            ControlEvent::TogglePlay => {
                self.send_engine(PlaybackCommand::SetPlaying(
                    self.state.is_playing,
                ));
            }
            ControlEvent::CycleMidiChannel => {
                self.send_engine(PlaybackCommand::SetMidiChannel(
                    self.state.midi_channel,
                ));
            }
            ControlEvent::Bpm(_) => {
                self.send_engine(PlaybackCommand::SetBPM(self.state.bpm));
            }
            // Changes the display only.
            ControlEvent::NextSlot => {}
            // Everything else changes what the patterns are.
            ControlEvent::Steps(_)
            | ControlEvent::Pulses(_)
            | ControlEvent::Phase(_)
            | ControlEvent::Pitch(_)
            | ControlEvent::Mix(_) => self.regenerate(),
        }

        self.publish();
    }

    /// Regenerate both patterns, mix them, and hand the result to the engine.
    fn regenerate(&mut self) {
        let left = self.state.slots[0].pattern();
        let right = self.state.slots[1].pattern();
        // Boxed: a PolyphonicSequence is ~32 KB inline (a no_std fixed-capacity
        // type), too big to memcpy through a channel.
        let mixed = Box::new(seq_core::mixer::mix(
            &left,
            &right,
            self.state.mix_ratio,
            &mut rand::rng(),
        ));
        self.send_engine(PlaybackCommand::LoadSequence(mixed));
    }

    /// Publish the frame the UI renders.
    ///
    /// `watch` is lossy-latest by design: a slow or absent UI cannot
    /// back-pressure the sequencer, it simply renders the newest frame.
    fn publish(&self) {
        self.tx_snapshot.send_replace(self.state.ui_snapshot());
    }

    fn handle_midi_command(&self, command: MidiCommand) {
        match command {
            MidiCommand::GetPorts { responder } => {
                match midi_utils::list_ports() {
                    Ok(ports) => {
                        if responder.send(ports).is_err() {
                            warn!("Unable to send MIDI output ports.");
                        }
                    }
                    Err(e) => error!("Unable to list MIDI output ports: {e}"),
                }
            }
            MidiCommand::SetPort { out_port } => {
                info!("Connecting to MIDI output port {out_port}");
                match midi_utils::create_connection(&out_port) {
                    Ok(connection) => {
                        self.send_engine(PlaybackCommand::SetOutputConnection(
                            Box::new(MidirSink(connection)),
                        ));
                    }
                    Err(e) => {
                        error!(
                            "Unable to connect to MIDI port {out_port}: {e}"
                        );
                    }
                }
            }
        }
    }

    fn send_engine(&self, command: PlaybackCommand) {
        if let Err(e) = self.tx_engine.send(command) {
            error!("Error sending command to PlaybackEngine: {e}");
        }
    }
}

/// The tempo the engine should be running at, for callers wiring one up.
#[must_use]
pub fn engine_bpm_milli(state: &SequencerState) -> u32 {
    bpm_to_milli(state.bpm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{Receiver as SyncReceiver, channel as sync_channel};

    struct Harness {
        sequencer: Option<Sequencer>,
        tx_control: Option<mpsc::Sender<ControlEvent>>,
        tx_midi: Option<mpsc::Sender<MidiCommand>>,
        tx_step: watch::Sender<usize>,
        rx_engine: SyncReceiver<PlaybackCommand>,
        rx_snapshot: watch::Receiver<UiSnapshot>,
    }

    impl Harness {
        fn new() -> Self {
            let (tx_control, rx_control) = mpsc::channel(16);
            let (tx_midi, rx_midi) = mpsc::channel(16);
            let (tx_step, rx_step) = watch::channel(0);
            let (tx_engine, rx_engine) = sync_channel();
            let (tx_snapshot, rx_snapshot) =
                watch::channel(UiSnapshot::default());

            let sequencer = Sequencer::new(
                SequencerState::default(),
                rx_control,
                rx_midi,
                rx_step.clone(),
                tx_engine,
                tx_snapshot,
            );

            Self {
                sequencer: Some(sequencer),
                tx_control: Some(tx_control),
                tx_midi: Some(tx_midi),
                tx_step,
                rx_engine,
                rx_snapshot,
            }
        }

        fn engine_commands(&self) -> Vec<PlaybackCommand> {
            self.rx_engine.try_iter().collect()
        }

        fn sequencer(&mut self) -> &mut Sequencer {
            self.sequencer.as_mut().unwrap()
        }

        /// Hand the task off to be driven, keeping the channel ends.
        fn take_sequencer(&mut self) -> Sequencer {
            self.sequencer.take().unwrap()
        }

        fn snapshot(&mut self) -> UiSnapshot {
            *self.rx_snapshot.borrow_and_update()
        }

        async fn send(&self, event: ControlEvent) {
            self.tx_control.as_ref().unwrap().send(event).await.unwrap();
        }

        /// Close the control channel, which is how `run` is asked to stop.
        fn close(&mut self) {
            self.tx_control = None;
        }

        /// Wait until the published frame satisfies `predicate`.
        async fn wait_for(&mut self, predicate: impl Fn(&UiSnapshot) -> bool) {
            loop {
                if predicate(&self.rx_snapshot.borrow_and_update()) {
                    return;
                }
                self.rx_snapshot.changed().await.unwrap();
            }
        }
    }

    fn is_load(command: &PlaybackCommand) -> bool {
        matches!(command, PlaybackCommand::LoadSequence(_))
    }

    #[test]
    fn test_pattern_changes_reach_the_engine() {
        let mut harness = Harness::new();

        harness.sequencer().handle_control(ControlEvent::Pulses(1));

        assert!(
            harness.engine_commands().iter().any(is_load),
            "a pulse change must produce a new sequence"
        );
    }

    /// Switching the displayed slot changes nothing audible, so it must not
    /// rebuild and reload the sequence — a reload releases sounding notes.
    #[test]
    fn test_display_only_changes_do_not_reload_the_sequence() {
        let mut harness = Harness::new();

        harness.sequencer().handle_control(ControlEvent::NextSlot);

        assert!(
            !harness.engine_commands().iter().any(is_load),
            "switching slots must not reload the sequence"
        );
    }

    #[test]
    fn test_transport_controls_reach_the_engine() {
        let mut harness = Harness::new();

        harness.sequencer().handle_control(ControlEvent::TogglePlay);
        harness.sequencer().handle_control(ControlEvent::Bpm(5));
        harness
            .sequencer()
            .handle_control(ControlEvent::CycleMidiChannel);

        let commands = harness.engine_commands();
        assert!(
            commands
                .iter()
                .any(|c| matches!(c, PlaybackCommand::SetPlaying(true)))
        );
        assert!(commands.iter().any(|c| {
            matches!(c, PlaybackCommand::SetBPM(b) if (*b - 125.0).abs() < 1e-9)
        }));
        assert!(
            commands
                .iter()
                .any(|c| matches!(c, PlaybackCommand::SetMidiChannel(1)))
        );
    }

    #[test]
    fn test_every_control_event_publishes_a_snapshot() {
        let mut harness = Harness::new();

        harness.sequencer().handle_control(ControlEvent::TogglePlay);

        assert!(
            harness.rx_snapshot.has_changed().unwrap(),
            "the UI must be told about every change"
        );
        assert!(harness.snapshot().playing);
    }

    #[tokio::test]
    async fn test_run_applies_control_events_and_exits_cleanly() {
        let mut harness = Harness::new();
        harness.send(ControlEvent::TogglePlay).await;
        harness.send(ControlEvent::Bpm(10)).await;
        harness.close();

        // Returns when its inputs close, rather than needing to be killed.
        harness.take_sequencer().run().await;

        let snapshot = harness.snapshot();
        assert!(snapshot.playing);
        assert_eq!(snapshot.bpm_milli, 130_000);
    }

    #[tokio::test]
    async fn test_step_updates_reach_the_snapshot() {
        let mut harness = Harness::new();
        let mut sequencer = harness.take_sequencer();
        let running = tokio::spawn(async move { sequencer.run().await });

        harness.tx_step.send_replace(7);
        harness.wait_for(|snapshot| snapshot.step_index == 7).await;

        harness.close();
        running.await.unwrap();
    }

    /// A closed channel is permanently ready, so `select!` would hand it the
    /// race every time. A dead MIDI channel must retire, not starve the live
    /// control channel — otherwise the task spins at 100% CPU and stops
    /// responding.
    #[tokio::test]
    async fn test_a_closed_input_does_not_starve_the_others() {
        let mut harness = Harness::new();
        harness.tx_midi = None;

        let mut sequencer = harness.take_sequencer();
        let running = tokio::spawn(async move { sequencer.run().await });

        // Each event is awaited before the next is sent, so by the second one
        // the closed MIDI channel is the *only* branch ready. A task that
        // retires it carries on; one that does not has already shut down.
        harness.send(ControlEvent::TogglePlay).await;
        harness.wait_for(|snapshot| snapshot.playing).await;

        harness.send(ControlEvent::Bpm(10)).await;
        harness
            .wait_for(|snapshot| snapshot.bpm_milli == 130_000)
            .await;

        harness.close();
        running.await.unwrap();
    }
}

//! The synchronous driver around [`Transport`].
//!
//! Everything here is the *platform* half: the command channel, the keyboard
//! poll and the sleep. The musical half lives in [`seq_core::Transport`],
//! which this type simply feeds with clock readings.

use device_query::{DeviceQuery, DeviceState, Keycode};
use log::{error, info};
use seq_core::{Clock, MidiSink, SystemClock, Transport};
use std::collections::HashSet;
use std::fmt;
use std::{sync::mpsc::Receiver, time::Duration};
use tokio::sync::mpsc::UnboundedSender;

use crate::playback::state::{PlaybackCommand, PlaybackStatus};

/// How long the driver sleeps between updates. Sets the playback timing
/// resolution, and with it the audible jitter floor.
const TICK_INTERVAL: Duration = Duration::from_millis(1);

/// The output a [`PlaybackEngine`] writes to.
///
/// Boxed rather than a type parameter because the output is swapped at runtime
/// when the user picks a different MIDI port.
pub type BoxedSink = Box<dyn MidiSink + Send>;

/// The transport keeps tempo in integer milli-BPM; the UI deals in `f64`.
fn bpm_to_milli(bpm: f64) -> u32 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to a u32-representable, non-negative range first"
    )]
    {
        (bpm.clamp(0.0, 4_000_000.0) * 1000.0).round() as u32
    }
}

pub struct PlaybackEngine<C: Clock = SystemClock> {
    transport: Transport<BoxedSink>,
    clock: C,

    rx_command: Receiver<PlaybackCommand>,
    tx_status: UnboundedSender<PlaybackStatus>,

    /// Last step index reported to the GUI, so `NotePlayed` is sent on change
    /// rather than on a timer of its own.
    last_reported_step: Option<usize>,
}

/// Hand-written because the clock carries no `Debug` bound; forwards to
/// [`Transport`], which holds everything worth looking at.
impl<C: Clock> fmt::Debug for PlaybackEngine<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlaybackEngine")
            .field("transport", &self.transport)
            .field("last_reported_step", &self.last_reported_step)
            .finish_non_exhaustive()
    }
}

impl PlaybackEngine<SystemClock> {
    #[must_use]
    pub fn new(
        rx_command: Receiver<PlaybackCommand>,
        tx_status: UnboundedSender<PlaybackStatus>,
        sink: BoxedSink,
    ) -> Self {
        Self::with_clock(rx_command, tx_status, sink, SystemClock::new())
    }
}

impl<C: Clock> PlaybackEngine<C> {
    pub fn with_clock(
        rx_command: Receiver<PlaybackCommand>,
        tx_status: UnboundedSender<PlaybackStatus>,
        sink: BoxedSink,
        clock: C,
    ) -> Self {
        Self {
            transport: Transport::new(sink),
            clock,
            rx_command,
            tx_status,
            last_reported_step: None,
        }
    }

    pub fn run(mut self) {
        info!("Starting synchronous playback engine");

        let device_state = DeviceState::new();
        let mut last_keys = HashSet::new();

        loop {
            self.handle_commands();

            let keys: HashSet<Keycode> =
                device_state.get_keys().into_iter().collect();
            if keys != last_keys {
                let pressed: Vec<_> =
                    keys.difference(&last_keys).copied().collect();
                self.handle_keys(&pressed);
                last_keys = keys;
            }

            self.update();

            std::thread::sleep(TICK_INTERVAL);
        }
    }

    /// One iteration of the driver loop, minus the keyboard poll — which needs
    /// a display server, and so cannot run under test.
    fn update(&mut self) {
        self.transport.advance_to(self.clock.now_us());
        self.report_step();
    }

    fn handle_commands(&mut self) {
        while let Ok(command) = self.rx_command.try_recv() {
            match command {
                PlaybackCommand::LoadSequence(sequence) => {
                    info!(
                        "Engine received new sequence of {} events",
                        sequence.events().len()
                    );
                    self.transport.load_sequence(*sequence);
                }
                PlaybackCommand::SetMidiChannel(channel) => {
                    self.transport.set_midi_channel(channel);
                }
                PlaybackCommand::SetBPM(bpm) => {
                    self.transport.set_bpm_milli(bpm_to_milli(bpm));
                }
                PlaybackCommand::SetOutputConnection(sink) => {
                    self.transport.set_sink(sink);
                }
            }
        }
    }

    /// Playback keys are handled here rather than round-tripping through
    /// `PlaybackHandler`, to keep the response immediate.
    fn handle_keys(&mut self, pressed: &[Keycode]) {
        for key in pressed {
            if *key == Keycode::Space {
                self.transport.set_playing(!self.transport.is_playing());
            }
        }

        if let Err(e) = self
            .tx_status
            .send(PlaybackStatus::InputChanged(pressed.to_vec()))
        {
            error!("Error sending input changes to PlaybackHandler: {e}");
        }
    }

    fn report_step(&mut self) {
        let step = self.transport.current_step();
        if self.last_reported_step == Some(step) {
            return;
        }
        self.last_reported_step = Some(step);

        if let Err(e) = self.tx_status.send(PlaybackStatus::NotePlayed(step)) {
            error!("Error sending PlaybackStatus: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seq_core::{
        EventVec, ManualClock, MidiEventType, PolyphonicSequence,
        RecordingSink, TICKS_PER_STEP, TimedEvent,
    };
    use std::sync::mpsc::{Sender as SyncSender, channel as sync_channel};
    use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

    /// At 120 BPM a sixteenth-note step is 125 ms.
    const STEP_US: u64 = 125_000;

    struct Harness {
        engine: PlaybackEngine<ManualClock>,
        commands: SyncSender<PlaybackCommand>,
        status: UnboundedReceiver<PlaybackStatus>,
        clock: ManualClock,
        sink: RecordingSink,
    }

    impl Harness {
        fn new() -> Self {
            let (commands, rx_command) = sync_channel();
            let (tx_status, status) = unbounded_channel();
            let clock = ManualClock::new();
            let sink = RecordingSink::new();

            let engine = PlaybackEngine::with_clock(
                rx_command,
                tx_status,
                Box::new(sink.clone()),
                clock.clone(),
            );

            Self {
                engine,
                commands,
                status,
                clock,
                sink,
            }
        }

        fn send(&self, command: PlaybackCommand) {
            self.commands.send(command).unwrap();
            // Commands are only read inside the loop.
        }

        /// Run the driver loop for `steps` sixteenth notes.
        fn run_steps(&mut self, steps: u64) {
            for _ in 0..steps * STEP_US / 1_000 {
                self.engine.handle_commands();
                self.clock.advance_us(1_000);
                self.engine.update();
            }
        }

        fn statuses(&mut self) -> Vec<PlaybackStatus> {
            let mut out = Vec::new();
            while let Ok(status) = self.status.try_recv() {
                out.push(status);
            }
            out
        }

        fn steps_reported(&mut self) -> Vec<usize> {
            self.statuses()
                .into_iter()
                .filter_map(|status| match status {
                    PlaybackStatus::NotePlayed(step) => Some(step),
                    PlaybackStatus::InputChanged(_) => None,
                })
                .collect()
        }
    }

    fn sequence(pitches: &[u8]) -> PolyphonicSequence {
        let mut events = EventVec::new();
        for (step, &pitch) in pitches.iter().enumerate() {
            let tick = u32::try_from(step).unwrap() * TICKS_PER_STEP;
            events
                .push(TimedEvent {
                    tick,
                    event: MidiEventType::NoteOn {
                        pitch,
                        velocity: 100,
                    },
                })
                .unwrap();
            events
                .push(TimedEvent {
                    tick: tick + TICKS_PER_STEP - 1,
                    event: MidiEventType::NoteOff { pitch },
                })
                .unwrap();
        }
        PolyphonicSequence::new(
            events,
            u32::try_from(pitches.len()).unwrap() * TICKS_PER_STEP,
        )
    }

    #[test]
    fn test_commands_reach_the_transport() {
        let mut harness = Harness::new();

        harness.send(PlaybackCommand::SetBPM(90.0));
        harness.send(PlaybackCommand::SetMidiChannel(7));
        harness.engine.handle_commands();

        assert_eq!(harness.engine.transport.bpm_milli(), 90_000);
        assert_eq!(harness.engine.transport.midi_channel(), 7);
    }

    #[test]
    fn test_fractional_bpm_survives_the_conversion() {
        assert_eq!(bpm_to_milli(120.5), 120_500);
        assert_eq!(bpm_to_milli(0.0), 0);
        assert_eq!(bpm_to_milli(-10.0), 0, "negative tempo clamps to zero");
    }

    #[test]
    fn test_loaded_sequence_plays() {
        let mut harness = Harness::new();
        harness
            .send(PlaybackCommand::LoadSequence(Box::new(sequence(&[60, 62]))));
        harness.engine.handle_commands();
        harness.engine.transport.set_playing(true);

        harness.run_steps(2);

        let pitches: Vec<u8> = harness
            .sink
            .messages()
            .iter()
            .filter(|m| m[0] & 0xF0 == 0x90)
            .map(|m| m[1])
            .collect();
        assert_eq!(pitches, vec![60, 62]);
    }

    /// The GUI's play head used to be driven by a tick counter kept alongside
    /// the play position; it is now derived from it, so it cannot drift out of
    /// step or report the same step twice.
    #[test]
    fn test_each_step_is_reported_exactly_once() {
        let mut harness = Harness::new();
        harness.send(PlaybackCommand::LoadSequence(Box::new(sequence(&[
            60, 62, 64, 65,
        ]))));
        harness.engine.handle_commands();
        harness.engine.transport.set_playing(true);

        harness.run_steps(4);

        assert_eq!(harness.steps_reported(), vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_paused_engine_reports_one_step_and_plays_nothing() {
        let mut harness = Harness::new();
        harness
            .send(PlaybackCommand::LoadSequence(Box::new(sequence(&[60, 62]))));
        harness.engine.handle_commands();

        harness.run_steps(4);

        // Not `is_empty`: loading a sequence legitimately emits the
        // all-notes-off pair, whether or not anything was sounding.
        let notes: Vec<_> = harness
            .sink
            .messages()
            .into_iter()
            .filter(|m| matches!(m[0] & 0xF0, 0x80 | 0x90))
            .collect();
        assert!(notes.is_empty(), "a paused engine played {notes:?}");
        assert_eq!(
            harness.steps_reported(),
            vec![0],
            "the initial position is reported once, then nothing moves"
        );
    }

    #[test]
    fn test_switching_output_moves_playback_to_the_new_sink() {
        let mut harness = Harness::new();
        harness
            .send(PlaybackCommand::LoadSequence(Box::new(sequence(&[60, 62]))));
        harness.engine.handle_commands();
        harness.engine.transport.set_playing(true);

        let replacement = RecordingSink::new();
        harness.send(PlaybackCommand::SetOutputConnection(Box::new(
            replacement.clone(),
        )));
        harness.run_steps(2);

        assert!(!replacement.is_empty(), "new sink received nothing");
    }
}

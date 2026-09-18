//! The synchronous driver around [`Transport`].
//!
//! Everything here is the *platform* half: the command channel and the
//! waiting. The musical half lives in [`seq_core::Transport`], which this type
//! feeds with clock readings.
//!
//! The loop does not poll. It asks the transport when it next needs attention
//! and blocks on its command channel until then, so a paused sequencer costs
//! nothing and a playing one wakes once per event rather than a thousand times
//! a second.

use log::info;
use seq_core::{Clock, MidiSink, SystemClock, Transport};
use std::fmt;
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::time::Duration;
use tokio::sync::watch;

use crate::playback::state::PlaybackCommand;
use crate::state::bpm_to_milli;

/// How close to a deadline the coarse sleep stops and a spin takes over.
///
/// `recv_timeout` is only as punctual as the OS scheduler, and a late wakeup
/// is a late note. Sleeping to within this margin and spinning the rest keeps
/// jitter under the ~1 ms a MIDI message takes on the wire, while leaving the
/// thread asleep for almost all of the interval.
const SPIN_MARGIN_US: u64 = 1_500;

/// The output a [`PlaybackEngine`] writes to.
///
/// Boxed rather than a type parameter because the output is swapped at runtime
/// when the user picks a different MIDI port.
pub type BoxedSink = Box<dyn MidiSink + Send>;

/// Why a wait ended.
enum Wake {
    Command(PlaybackCommand),
    /// The transport's deadline arrived.
    Elapsed,
    /// Every command sender is gone.
    Shutdown,
}

pub struct PlaybackEngine<C: Clock = SystemClock> {
    transport: Transport<BoxedSink>,
    clock: C,

    rx_command: Receiver<PlaybackCommand>,
    /// The play head, for the display. `watch` because only the newest value
    /// matters — a UI that misses a step should skip it, not queue it.
    tx_step: watch::Sender<usize>,
}

/// Hand-written because the clock carries no `Debug` bound; forwards to
/// [`Transport`], which holds everything worth looking at.
impl<C: Clock> fmt::Debug for PlaybackEngine<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlaybackEngine")
            .field("transport", &self.transport)
            .finish_non_exhaustive()
    }
}

impl PlaybackEngine<SystemClock> {
    #[must_use]
    pub fn new(
        rx_command: Receiver<PlaybackCommand>,
        tx_step: watch::Sender<usize>,
        sink: BoxedSink,
    ) -> Self {
        Self::with_clock(rx_command, tx_step, sink, SystemClock::new())
    }
}

impl<C: Clock> PlaybackEngine<C> {
    pub fn with_clock(
        rx_command: Receiver<PlaybackCommand>,
        tx_step: watch::Sender<usize>,
        sink: BoxedSink,
        clock: C,
    ) -> Self {
        Self {
            transport: Transport::new(sink),
            clock,
            rx_command,
            tx_step,
        }
    }

    pub fn run(mut self) {
        info!("Starting synchronous playback engine");

        loop {
            self.update();

            match self.wait() {
                Wake::Command(command) => {
                    self.apply(command);
                    // Anything that arrived alongside it, before recomputing
                    // the deadline.
                    while let Ok(command) = self.rx_command.try_recv() {
                        self.apply(command);
                    }
                }
                Wake::Elapsed => {}
                Wake::Shutdown => break,
            }
        }

        // Nothing should still be sounding when the program ends.
        self.transport.set_playing(false);
        info!("Playback engine stopped");
    }

    /// Advance the play head and report where it landed.
    fn update(&mut self) {
        self.transport.advance_to(self.clock.now_us());

        let step = self.transport.current_step();
        if *self.tx_step.borrow() != step {
            self.tx_step.send_replace(step);
        }
    }

    /// Block until the transport needs attention or a command arrives.
    fn wait(&self) -> Wake {
        let Some(until) = self.transport.time_to_next_wakeup_us() else {
            // Nothing is scheduled — a stopped transport has no deadline, so
            // sleep properly until something asks for work.
            return match self.rx_command.recv() {
                Ok(command) => Wake::Command(command),
                Err(_) => Wake::Shutdown,
            };
        };

        let deadline = self.clock.now_us().saturating_add(until);
        loop {
            let remaining = deadline.saturating_sub(self.clock.now_us());
            if remaining == 0 {
                return Wake::Elapsed;
            }

            if remaining > SPIN_MARGIN_US {
                let timeout = Duration::from_micros(remaining - SPIN_MARGIN_US);
                match self.rx_command.recv_timeout(timeout) {
                    Ok(command) => return Wake::Command(command),
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => {
                        return Wake::Shutdown;
                    }
                }
            } else {
                // Final approach: too close to trust the scheduler.
                match self.rx_command.try_recv() {
                    Ok(command) => return Wake::Command(command),
                    Err(TryRecvError::Empty) => std::hint::spin_loop(),
                    Err(TryRecvError::Disconnected) => return Wake::Shutdown,
                }
            }
        }
    }

    fn apply(&mut self, command: PlaybackCommand) {
        match command {
            PlaybackCommand::LoadSequence(sequence) => {
                info!(
                    "Engine received new sequence of {} events",
                    sequence.events().len()
                );
                self.transport.load_sequence(*sequence);
            }
            PlaybackCommand::SetPlaying(playing) => {
                self.transport.set_playing(playing);
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

#[cfg(test)]
mod tests {
    use super::*;
    use seq_core::{
        EventVec, ManualClock, MidiEventType, PolyphonicSequence,
        RecordingSink, TICKS_PER_STEP, TimedEvent,
    };
    use std::sync::mpsc::{Sender as SyncSender, channel as sync_channel};

    /// At 120 BPM a sixteenth-note step is 125 ms.
    const STEP_US: u64 = 125_000;

    struct Harness {
        engine: PlaybackEngine<ManualClock>,
        commands: SyncSender<PlaybackCommand>,
        steps: watch::Receiver<usize>,
        clock: ManualClock,
        sink: RecordingSink,
    }

    impl Harness {
        fn new() -> Self {
            let (commands, rx_command) = sync_channel();
            let (tx_step, steps) = watch::channel(0);
            let clock = ManualClock::new();
            let sink = RecordingSink::new();

            let engine = PlaybackEngine::with_clock(
                rx_command,
                tx_step,
                Box::new(sink.clone()),
                clock.clone(),
            );

            Self {
                engine,
                commands,
                steps,
                clock,
                sink,
            }
        }

        fn send(&mut self, command: PlaybackCommand) {
            self.commands.send(command).unwrap();
            self.drain_commands();
        }

        fn drain_commands(&mut self) {
            while let Ok(command) = self.engine.rx_command.try_recv() {
                self.engine.apply(command);
            }
        }

        /// Run the driver loop for `steps` sixteenth notes, in the 1 ms slices
        /// a coarse scheduler would produce.
        fn run_steps(&mut self, steps: u64) {
            for _ in 0..steps * STEP_US / 1_000 {
                self.drain_commands();
                self.clock.advance_us(1_000);
                self.engine.update();
            }
        }

        fn note_ons(&self) -> Vec<u8> {
            self.sink
                .messages()
                .iter()
                .filter(|m| m[0] & 0xF0 == 0x90)
                .map(|m| m[1])
                .collect()
        }
    }

    fn sequence(pitches: &[u8]) -> Box<PolyphonicSequence> {
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
        Box::new(PolyphonicSequence::new(
            events,
            u32::try_from(pitches.len()).unwrap() * TICKS_PER_STEP,
        ))
    }

    #[test]
    fn test_commands_reach_the_transport() {
        let mut harness = Harness::new();

        harness.send(PlaybackCommand::SetBPM(90.0));
        harness.send(PlaybackCommand::SetMidiChannel(7));

        assert_eq!(harness.engine.transport.bpm_milli(), 90_000);
        assert_eq!(harness.engine.transport.midi_channel(), 7);
    }

    /// Playback is now started by command rather than by the engine reading
    /// the keyboard itself.
    #[test]
    fn test_set_playing_starts_and_stops_playback() {
        let mut harness = Harness::new();
        harness.send(PlaybackCommand::LoadSequence(sequence(&[60, 62])));

        assert!(!harness.engine.transport.is_playing());
        harness.send(PlaybackCommand::SetPlaying(true));
        assert!(harness.engine.transport.is_playing());

        harness.run_steps(1);
        assert_eq!(harness.note_ons(), vec![60]);

        harness.send(PlaybackCommand::SetPlaying(false));
        assert!(!harness.engine.transport.is_playing());
    }

    #[test]
    fn test_loaded_sequence_plays() {
        let mut harness = Harness::new();
        harness.send(PlaybackCommand::LoadSequence(sequence(&[60, 62])));
        harness.send(PlaybackCommand::SetPlaying(true));

        harness.run_steps(2);

        assert_eq!(harness.note_ons(), vec![60, 62]);
    }

    /// The GUI's play head is derived from the play position, so it cannot
    /// drift out of step or report the same step twice.
    #[test]
    fn test_each_step_is_published_once() {
        let mut harness = Harness::new();
        harness
            .send(PlaybackCommand::LoadSequence(sequence(&[60, 62, 64, 65])));
        harness.send(PlaybackCommand::SetPlaying(true));

        let mut seen = vec![*harness.steps.borrow_and_update()];
        for _ in 0..4 * STEP_US / 1_000 {
            harness.clock.advance_us(1_000);
            harness.engine.update();
            if harness.steps.has_changed().unwrap() {
                seen.push(*harness.steps.borrow_and_update());
            }
        }

        assert_eq!(seen, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_paused_engine_plays_nothing() {
        let mut harness = Harness::new();
        harness.send(PlaybackCommand::LoadSequence(sequence(&[60, 62])));

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
    }

    #[test]
    fn test_switching_output_moves_playback_to_the_new_sink() {
        let mut harness = Harness::new();
        harness.send(PlaybackCommand::LoadSequence(sequence(&[60, 62])));
        harness.send(PlaybackCommand::SetPlaying(true));

        let replacement = RecordingSink::new();
        harness.send(PlaybackCommand::SetOutputConnection(Box::new(
            replacement.clone(),
        )));
        harness.run_steps(2);

        assert!(!replacement.is_empty(), "new sink received nothing");
    }

    /// Closing the command channel ends the loop instead of leaving the thread
    /// spinning, and anything still sounding is released on the way out.
    #[test]
    fn test_run_exits_when_its_commands_close() {
        let (commands, rx_command) = sync_channel();
        let (tx_step, _steps) = watch::channel(0);
        let sink = RecordingSink::new();
        let engine = PlaybackEngine::with_clock(
            rx_command,
            tx_step,
            Box::new(sink.clone()),
            ManualClock::new(),
        );

        commands
            .send(PlaybackCommand::LoadSequence(sequence(&[60])))
            .unwrap();
        drop(commands);

        // Terminates: with no senders left the wait returns Shutdown.
        engine.run();

        assert!(!sink.is_empty(), "the release burst should have been sent");
    }
}

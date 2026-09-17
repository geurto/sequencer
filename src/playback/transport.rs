//! The sequencing core: play position, event dispatch and note bookkeeping.
//!
//! [`Transport`] owns no channels, no threads and no input device — it is
//! driven entirely by [`Transport::advance_to`] and writes through a
//! [`MidiSink`]. That makes playback a function of (sequence, clock readings)
//! to MIDI bytes, which is what makes it testable, and it is the part that
//! survives into a platform-independent core later.

use std::fmt;

use log::error;

use crate::playback::sink::MidiSink;
use crate::playback::state::{
    MidiEventType, PolyphonicSequence, TICKS_PER_QUARTER_NOTE, TICKS_PER_STEP,
    TimedEvent,
};

const NOTE_ON: u8 = 0x90;
const NOTE_OFF: u8 = 0x80;
const CONTROL_CHANGE: u8 = 0xB0;
const CC_ALL_SOUND_OFF: u8 = 120;
const CC_ALL_NOTES_OFF: u8 = 123;

const MIDI_CHANNELS: u8 = 16;
const MAX_MIDI_PITCH: u8 = 127;
const MAX_MIDI_VELOCITY: u8 = 127;

const SECONDS_PER_MINUTE: f64 = 60.0;
const MICROS_PER_SECOND: f64 = 1_000_000.0;

pub struct Transport<S: MidiSink> {
    sink: S,

    is_playing: bool,
    sequence: PolyphonicSequence,
    next_event_index: usize,
    current_tick: f64,
    bpm: f64,
    midi_channel: u8,

    /// Bit `c` of `sounding[p]` is set while pitch `p` is held on channel `c`.
    ///
    /// Tracked so notes can be released *explicitly*: CC 123 is advisory and
    /// some synths ignore it, and a mid-note channel switch would otherwise
    /// orphan whatever is still held on the previous channel.
    sounding: [u16; 128],

    /// Previous reading from the clock, or `None` before the first one.
    last_now_us: Option<u64>,
}

/// Hand-written rather than derived: the sink is a trait object with no `Debug`
/// bound, and the interesting content is the musical state, not the plumbing.
impl<S: MidiSink> fmt::Debug for Transport<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transport")
            .field("is_playing", &self.is_playing)
            .field("bpm", &self.bpm)
            .field("midi_channel", &self.midi_channel)
            .field("step", &self.current_step())
            .field("tick", &self.current_tick)
            .field("events", &self.sequence.events().len())
            .field("sounding", &self.sounding_notes().count())
            .finish_non_exhaustive()
    }
}

impl<S: MidiSink> Transport<S> {
    pub fn new(sink: S) -> Self {
        Self {
            sink,
            is_playing: false,
            sequence: PolyphonicSequence::default(),
            next_event_index: 0,
            current_tick: 0.0,
            bpm: 120.0,
            midi_channel: 0,
            sounding: [0; 128],
            last_now_us: None,
        }
    }

    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// Start or stop playback. Stopping releases everything still sounding, so
    /// a pause cannot leave a note held indefinitely.
    pub fn set_playing(&mut self, is_playing: bool) {
        if is_playing == self.is_playing {
            return;
        }
        self.is_playing = is_playing;
        if !is_playing {
            self.release_all_notes();
        }
    }

    #[must_use]
    pub fn bpm(&self) -> f64 {
        self.bpm
    }

    pub fn set_bpm(&mut self, bpm: f64) {
        self.bpm = bpm.max(0.0);
    }

    #[must_use]
    pub fn midi_channel(&self) -> u8 {
        self.midi_channel
    }

    /// Switch output channel, releasing anything held on the previous one —
    /// those notes could otherwise never be addressed again.
    pub fn set_midi_channel(&mut self, channel: u8) {
        let channel = channel % MIDI_CHANNELS;
        if channel == self.midi_channel {
            return;
        }
        self.release_all_notes();
        self.midi_channel = channel;
    }

    /// Replace the output, releasing held notes through the old one first.
    pub fn set_sink(&mut self, sink: S) {
        self.release_all_notes();
        self.sink = sink;
    }

    /// Which sixteenth-note step the play head is on.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "`wrap_around` keeps the head inside the sequence, and a \
                  float-to-int cast saturates rather than wrapping, so a \
                  out-of-range head would clamp to a valid step rather than \
                  produce a bogus index"
    )]
    pub fn current_step(&self) -> usize {
        (self.current_tick / f64::from(TICKS_PER_STEP)) as usize
    }

    /// Pitches currently held, as `(channel, pitch)` pairs. Test/telemetry aid.
    pub fn sounding_notes(&self) -> impl Iterator<Item = (u8, u8)> + '_ {
        // Pitches are counted as u8 rather than indexed by usize, so no cast is
        // needed to get back to a MIDI pitch.
        (0..=MAX_MIDI_PITCH).zip(self.sounding.iter()).flat_map(
            |(pitch, held)| {
                (0..MIDI_CHANNELS)
                    .filter(move |channel| held & (1 << channel) != 0)
                    .map(move |channel| (channel, pitch))
            },
        )
    }

    /// Swap in a new sequence, keeping the play head at the same relative
    /// position within the loop.
    pub fn load_sequence(&mut self, sequence: PolyphonicSequence) {
        self.release_all_notes();

        let old_total = self.sequence.total_ticks();
        let new_total = sequence.total_ticks();

        if old_total > 0 && new_total > 0 {
            let length_ratio = f64::from(new_total) / f64::from(old_total);
            self.current_tick *= length_ratio;
        } else {
            self.current_tick = 0.0;
        }

        self.sequence = sequence;
        self.reseek();
    }

    /// Advance the play head to `now_us` and dispatch everything it passed.
    ///
    /// Safe to call with an unchanged or decreasing reading: time never moves
    /// backwards, it just does not move.
    pub fn advance_to(&mut self, now_us: u64) {
        let previous = self.last_now_us.replace(now_us).unwrap_or(now_us);

        if !self.is_playing {
            return;
        }

        let delta_us = now_us.saturating_sub(previous);
        let ticks_per_us = (self.bpm / SECONDS_PER_MINUTE)
            * f64::from(TICKS_PER_QUARTER_NOTE)
            / MICROS_PER_SECOND;
        // Exact: f64 represents every integer below 2^53, and this is the gap
        // between two clock readings — microseconds, not an absolute timestamp.
        #[expect(
            clippy::cast_precision_loss,
            reason = "a delta large enough to lose precision is ~285 years"
        )]
        let delta_us = delta_us as f64;
        self.current_tick += delta_us * ticks_per_us;

        self.wrap_around();
        self.dispatch_due_events();
    }

    /// Bring the play head back inside the sequence after it ran off the end.
    fn wrap_around(&mut self) {
        let total_ticks = f64::from(self.sequence.total_ticks());
        if total_ticks <= 0.0 || self.current_tick < total_ticks {
            return;
        }

        // Discard whole loops rather than subtracting one length: a long stall
        // would otherwise leave the head beyond the end and replay the whole
        // sequence in a burst on each following call.
        let laps = (self.current_tick / total_ticks).floor();
        self.current_tick -= laps * total_ticks;
        self.next_event_index = 0;
    }

    fn dispatch_due_events(&mut self) {
        while let Some(&event) =
            self.sequence.events().get(self.next_event_index)
        {
            if f64::from(event.tick) > self.current_tick {
                break;
            }
            self.process_event(event);
            self.next_event_index += 1;
        }
    }

    /// Point the event cursor at the first event at or after the play head.
    fn reseek(&mut self) {
        self.next_event_index = self
            .sequence
            .events()
            .partition_point(|event| f64::from(event.tick) < self.current_tick);
    }

    fn process_event(&mut self, timed_event: TimedEvent) {
        let channel = self.midi_channel;
        let message = match timed_event.event {
            MidiEventType::NoteOn { pitch, velocity } => {
                let pitch = pitch.min(MAX_MIDI_PITCH);
                self.sounding[pitch as usize] |= 1 << channel;
                [NOTE_ON | channel, pitch, velocity.min(MAX_MIDI_VELOCITY)]
            }
            MidiEventType::NoteOff { pitch } => {
                let pitch = pitch.min(MAX_MIDI_PITCH);
                self.sounding[pitch as usize] &= !(1 << channel);
                [NOTE_OFF | channel, pitch, 0]
            }
        };

        self.send(&message);
    }

    /// Explicitly release every note this transport started and has not yet
    /// stopped, on whichever channel it was started on.
    pub fn release_all_notes(&mut self) {
        for pitch in 0..=MAX_MIDI_PITCH {
            let held = self.sounding[usize::from(pitch)];
            if held == 0 {
                continue;
            }
            for channel in 0..MIDI_CHANNELS {
                if held & (1 << channel) != 0 {
                    self.send(&[NOTE_OFF | channel, pitch, 0]);
                }
            }
            self.sounding[usize::from(pitch)] = 0;
        }

        // Belt and braces for anything this transport did not start itself
        // (e.g. a synth that missed an earlier Note-Off).
        let channel = self.midi_channel;
        self.send(&[CONTROL_CHANGE | channel, CC_ALL_NOTES_OFF, 0]);
        self.send(&[CONTROL_CHANGE | channel, CC_ALL_SOUND_OFF, 0]);
    }

    fn send(&mut self, message: &[u8]) {
        if let Err(e) = self.sink.send(message) {
            error!("MIDI send error: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playback::sink::{RecordingSink, SendError};
    use std::collections::HashMap;

    const BPM: f64 = 120.0;

    /// At 120 BPM a quarter note is 500 ms, so a sixteenth-note step is 125 ms.
    const STEP_US: u64 = 125_000;

    /// The driver loop's update interval; tests advance in the same slices so
    /// they exercise the same code path as production.
    const SLICE_US: u64 = 1_000;

    fn note_on(tick: u32, pitch: u8) -> TimedEvent {
        TimedEvent {
            tick,
            event: MidiEventType::NoteOn {
                pitch,
                velocity: 100,
            },
        }
    }

    fn note_off(tick: u32, pitch: u8) -> TimedEvent {
        TimedEvent {
            tick,
            event: MidiEventType::NoteOff { pitch },
        }
    }

    /// One note per step, each released just before the next begins.
    pub(super) fn sequence(pitches: &[u8]) -> PolyphonicSequence {
        let mut events = Vec::new();
        for (step, &pitch) in pitches.iter().enumerate() {
            let tick = u32::try_from(step).unwrap() * TICKS_PER_STEP;
            events.push(note_on(tick, pitch));
            events.push(note_off(tick + TICKS_PER_STEP - 1, pitch));
        }
        PolyphonicSequence::new(
            events,
            u32::try_from(pitches.len()).unwrap() * TICKS_PER_STEP,
        )
    }

    /// A single note held for the whole sequence, so the head is reliably
    /// mid-note when something interrupts it.
    fn sustained(pitch: u8, steps: u32) -> PolyphonicSequence {
        PolyphonicSequence::new(
            vec![
                note_on(0, pitch),
                note_off(steps * TICKS_PER_STEP - 1, pitch),
            ],
            steps * TICKS_PER_STEP,
        )
    }

    /// A transport that is already playing, with a recorder on its output.
    ///
    /// The clock is anchored *before* playback starts, mirroring the driver
    /// loop, which is already ticking while the transport sits paused.
    fn playing(
        sequence: PolyphonicSequence,
    ) -> (Transport<RecordingSink>, RecordingSink) {
        let recorder = RecordingSink::new();
        let mut transport = Transport::new(recorder.clone());
        transport.set_bpm(BPM);
        transport.load_sequence(sequence);
        transport.advance_to(0);
        transport.set_playing(true);
        recorder.clear(); // drop the release burst from load
        (transport, recorder)
    }

    /// Advance to the middle of step `step`, counted from the clock origin.
    ///
    /// Time is absolute rather than relative so that successive calls in one
    /// test do not compound, and lands mid-step to avoid the ambiguity of
    /// sitting exactly on a boundary. Advances in the slices the driver loop
    /// would use, so a long run exercises the same path as production.
    fn run_to_step(transport: &mut Transport<RecordingSink>, step: u64) {
        let target = step * STEP_US + STEP_US / 2;
        let start = transport.last_now_us.unwrap_or(0);
        assert!(target >= start, "run_to_step only moves forwards");

        for slice in 1..=(target - start) / SLICE_US {
            transport.advance_to(start + slice * SLICE_US);
        }
        transport.advance_to(target);
    }

    fn note_ons(recorder: &RecordingSink) -> Vec<u8> {
        recorder
            .messages()
            .iter()
            .filter(|m| m[0] & 0xF0 == NOTE_ON)
            .map(|m| m[1])
            .collect()
    }

    /// Net outstanding Note-Ons per pitch. Empty means everything that started
    /// was also stopped.
    pub(super) fn outstanding(recorder: &RecordingSink) -> HashMap<u8, i32> {
        let mut held: HashMap<u8, i32> = HashMap::new();
        for message in recorder.messages() {
            match message[0] & 0xF0 {
                NOTE_ON => *held.entry(message[1]).or_default() += 1,
                NOTE_OFF => *held.entry(message[1]).or_default() -= 1,
                _ => {}
            }
        }
        held.retain(|_, count| *count != 0);
        held
    }

    #[test]
    fn test_paused_transport_emits_nothing() {
        let recorder = RecordingSink::new();
        let mut transport = Transport::new(recorder.clone());
        transport.set_bpm(BPM);
        transport.load_sequence(sequence(&[60, 62, 64, 65]));
        recorder.clear();

        run_to_step(&mut transport, 8);

        assert!(
            recorder.is_empty(),
            "a paused transport sent {:?}",
            recorder.messages()
        );
    }

    #[test]
    fn test_plays_notes_in_order() {
        let (mut transport, recorder) = playing(sequence(&[60, 62, 64, 65]));

        run_to_step(&mut transport, 3);

        assert_eq!(note_ons(&recorder), vec![60, 62, 64, 65]);
    }

    #[test]
    fn test_first_note_fires_immediately() {
        let (mut transport, recorder) = playing(sequence(&[60, 62]));

        transport.advance_to(1);

        assert_eq!(note_ons(&recorder), vec![60]);
    }

    #[test]
    fn test_sequence_loops() {
        let (mut transport, recorder) = playing(sequence(&[60, 62]));

        run_to_step(&mut transport, 5);

        assert_eq!(note_ons(&recorder), vec![60, 62, 60, 62, 60, 62]);
    }

    #[test]
    fn test_notes_balance_over_many_loops() {
        let (mut transport, recorder) = playing(sequence(&[60, 61, 62, 64]));

        run_to_step(&mut transport, 400);
        transport.set_playing(false);

        assert!(!recorder.is_empty());
        assert!(
            outstanding(&recorder).is_empty(),
            "notes left hanging: {:?}",
            outstanding(&recorder)
        );
    }

    /// A stalled thread must not replay everything it missed in a burst — the
    /// head is brought back by whole loops, not one length at a time.
    #[test]
    fn test_long_stall_does_not_burst() {
        let (mut transport, recorder) = playing(sequence(&[60, 62]));

        // Twenty loops' worth of time in a single reading.
        transport.advance_to(40 * STEP_US);

        let struck = note_ons(&recorder).len();
        assert!(
            struck <= 2,
            "a single stalled update emitted {struck} Note-Ons"
        );
    }

    #[test]
    fn test_stall_leaves_the_head_inside_the_sequence() {
        let (mut transport, _recorder) = playing(sequence(&[60, 62]));

        transport.advance_to(40 * STEP_US);

        assert!(transport.current_step() < 2);
    }

    #[test]
    fn test_tempo_sets_the_rate() {
        let (mut slow, slow_recorder) = playing(sequence(&[60, 62, 64, 65]));
        run_to_step(&mut slow, 3);

        let (mut fast, fast_recorder) = playing(sequence(&[60, 62, 64, 65]));
        fast.set_bpm(BPM * 2.0);
        run_to_step(&mut fast, 3);

        assert_eq!(note_ons(&slow_recorder).len(), 4);
        assert_eq!(
            note_ons(&fast_recorder).len(),
            2 * note_ons(&slow_recorder).len(),
            "double tempo should cover twice the music in the same wall time"
        );
    }

    #[test]
    fn test_zero_tempo_does_not_advance() {
        let (mut transport, recorder) = playing(sequence(&[60, 62]));
        transport.set_bpm(0.0);

        run_to_step(&mut transport, 10);

        assert_eq!(
            note_ons(&recorder),
            vec![60],
            "only the note under the stationary head should fire"
        );
    }

    #[test]
    fn test_repeated_readings_do_not_advance_time() {
        let (mut transport, recorder) = playing(sequence(&[60, 62, 64, 65]));

        for _ in 0..100 {
            transport.advance_to(STEP_US);
        }

        assert_eq!(note_ons(&recorder), vec![60, 62]);
    }

    #[test]
    fn test_time_never_runs_backwards() {
        let (mut transport, recorder) = playing(sequence(&[60, 62, 64, 65]));

        transport.advance_to(2 * STEP_US);
        let before = transport.current_step();
        transport.advance_to(0);

        assert_eq!(transport.current_step(), before);
        assert_eq!(note_ons(&recorder), vec![60, 62, 64]);
    }

    #[test]
    fn test_empty_sequence_is_silent() {
        let (mut transport, recorder) =
            playing(PolyphonicSequence::new(Vec::new(), 0));

        run_to_step(&mut transport, 16);

        assert!(recorder.is_empty());
    }

    #[test]
    fn test_pause_releases_sounding_notes() {
        let (mut transport, recorder) = playing(sustained(60, 4));

        run_to_step(&mut transport, 0);
        assert_eq!(note_ons(&recorder), vec![60], "note should be sounding");

        transport.set_playing(false);

        assert!(
            outstanding(&recorder).is_empty(),
            "pausing left notes held: {:?}",
            outstanding(&recorder)
        );
        assert!(transport.sounding_notes().next().is_none());
    }

    #[test]
    fn test_channel_is_applied_to_messages() {
        let (mut transport, recorder) = playing(sequence(&[60]));
        transport.set_midi_channel(5);
        recorder.clear();

        run_to_step(&mut transport, 0);

        assert_eq!(recorder.messages()[0][0], NOTE_ON | 5);
    }

    #[test]
    fn test_channel_change_releases_on_the_old_channel() {
        let (mut transport, recorder) = playing(sustained(60, 4));
        run_to_step(&mut transport, 0);

        transport.set_midi_channel(9);

        assert!(
            recorder.messages().contains(&vec![NOTE_OFF, 60, 0]),
            "expected a Note-Off on the original channel, got {:?}",
            recorder.messages()
        );
        assert!(transport.sounding_notes().next().is_none());
    }

    #[test]
    fn test_channel_wraps_at_sixteen() {
        let mut transport = Transport::new(RecordingSink::new());

        transport.set_midi_channel(16);
        assert_eq!(transport.midi_channel(), 0);

        transport.set_midi_channel(17);
        assert_eq!(transport.midi_channel(), 1);
    }

    #[test]
    fn test_loading_a_sequence_releases_previous_notes() {
        let (mut transport, recorder) = playing(sustained(60, 4));
        run_to_step(&mut transport, 0);

        transport.load_sequence(sequence(&[72, 74]));

        assert!(
            outstanding(&recorder).is_empty(),
            "swapping sequences left notes held: {:?}",
            outstanding(&recorder)
        );
        assert!(transport.sounding_notes().next().is_none());
    }

    /// After a swap the cursor must point at the next event *ahead* of the play
    /// head, or the remainder of the bar replays from its beginning.
    #[test]
    fn test_loading_a_sequence_keeps_the_play_position() {
        let (mut transport, recorder) =
            playing(sequence(&[60, 61, 62, 63, 64, 65, 66, 67]));
        run_to_step(&mut transport, 4);
        recorder.clear();

        // Same length, different pitches: the head should carry on at step 5,
        // not restart at the top of the sequence.
        transport.load_sequence(sequence(&[70, 71, 72, 73, 74, 75, 76, 77]));
        run_to_step(&mut transport, 5);

        assert_eq!(note_ons(&recorder), vec![75]);
    }

    /// Swapping to a sequence of a different length moves the play head (it is
    /// rescaled to the same relative position), so the event cursor has to be
    /// re-seeked. Leaving it where it was makes the transport replay every
    /// event between the old cursor and the new head in a single burst.
    #[test]
    fn test_loading_a_longer_sequence_does_not_burst() {
        let (mut transport, recorder) = playing(sequence(&[60, 62]));
        run_to_step(&mut transport, 1);
        recorder.clear();

        // Three times as long, so the head is rescaled from step 1.5 to step
        // 4.5 — deliberately between two events, since a head landing exactly
        // on an event tick makes the result depend on float rounding.
        transport.load_sequence(sequence(&[70, 71, 72, 73, 74, 75]));
        run_to_step(&mut transport, 2);

        assert_eq!(
            note_ons(&recorder),
            vec![75],
            "expected playback to resume at the rescaled head, not replay up to it"
        );
    }

    #[test]
    fn test_replacing_the_sink_releases_through_the_old_one() {
        let (mut transport, old) = playing(sustained(60, 4));
        run_to_step(&mut transport, 0);

        let new = RecordingSink::new();
        transport.set_sink(new.clone());

        assert!(
            old.messages().contains(&vec![NOTE_OFF, 60, 0]),
            "release should go through the sink that started the note"
        );
        assert!(new.is_empty());
    }

    #[test]
    fn test_out_of_range_pitch_and_velocity_are_masked() {
        let recorder = RecordingSink::new();
        let mut transport = Transport::new(recorder.clone());
        transport.set_bpm(BPM);
        transport.load_sequence(PolyphonicSequence::new(
            vec![TimedEvent {
                tick: 0,
                event: MidiEventType::NoteOn {
                    pitch: 200,
                    velocity: 240,
                },
            }],
            TICKS_PER_STEP,
        ));
        transport.advance_to(0);
        transport.set_playing(true);
        recorder.clear();

        run_to_step(&mut transport, 0);

        let message = &recorder.messages()[0];
        assert!(
            message[1] <= MAX_MIDI_PITCH,
            "pitch {} not masked",
            message[1]
        );
        assert!(
            message[2] <= MAX_MIDI_VELOCITY,
            "velocity {} not masked",
            message[2]
        );
    }

    #[test]
    fn test_a_failing_sink_does_not_stop_playback() {
        let mut transport = Transport::new(RecordingSink::failing(
            SendError::Other("unplugged"),
        ));
        transport.set_bpm(BPM);
        transport.load_sequence(sequence(&[60, 62]));
        transport.advance_to(0);
        transport.set_playing(true);

        for slice in 1..=8 {
            transport.advance_to(slice * STEP_US);
        }

        assert!(transport.is_playing());
    }

    #[test]
    fn test_current_step_tracks_the_play_head() {
        let (mut transport, _recorder) = playing(sequence(&[60, 62, 64, 65]));

        assert_eq!(transport.current_step(), 0);
        run_to_step(&mut transport, 1);
        assert_eq!(transport.current_step(), 1);
        run_to_step(&mut transport, 3);
        assert_eq!(transport.current_step(), 3);
    }
}

#[cfg(test)]
mod property_tests {
    use super::tests::sequence;
    use super::*;
    use crate::playback::sink::RecordingSink;
    use proptest::prelude::*;
    use std::collections::BTreeSet;

    /// Which `(channel, pitch)` pairs a synth would still be sounding after
    /// replaying this message stream.
    ///
    /// Modelled as a set rather than a counter: re-striking a held pitch does
    /// not stack voices, so one Note-Off releases it however many Note-Ons
    /// preceded. A Note-On with velocity 0 is a Note-Off by convention.
    fn still_ringing(recorder: &RecordingSink) -> Vec<(u8, u8)> {
        let mut held = BTreeSet::new();
        for message in recorder.messages() {
            let (status, channel, pitch) =
                (message[0] & 0xF0, message[0] & 0x0F, message[1]);
            match status {
                NOTE_ON if message[2] > 0 => {
                    held.insert((channel, pitch));
                }
                NOTE_ON | NOTE_OFF => {
                    held.remove(&(channel, pitch));
                }
                _ => {}
            }
        }
        held.into_iter().collect()
    }

    /// Arbitrary, possibly ill-formed events: unbalanced, out of order, and
    /// with ticks past the end of the sequence.
    fn any_events() -> impl Strategy<Value = Vec<TimedEvent>> {
        proptest::collection::vec(
            (0u32..2_000, 0u8..=127, 0u8..=127, any::<bool>()).prop_map(
                |(tick, pitch, velocity, is_on)| TimedEvent {
                    tick,
                    event: if is_on {
                        MidiEventType::NoteOn { pitch, velocity }
                    } else {
                        MidiEventType::NoteOff { pitch }
                    },
                },
            ),
            0..40,
        )
    }

    proptest! {
        /// However the clock behaves — stalls, repeats, going backwards — the
        /// transport must not panic, and stopping must leave nothing sounding.
        /// A hung MIDI note is the worst failure this program has.
        #[test]
        fn prop_stopping_never_leaves_a_note_hanging(
            events in any_events(),
            total_ticks in 0u32..2_000,
            bpm in 0.0f64..400.0,
            readings in proptest::collection::vec(0u64..5_000_000, 0..60),
        ) {
            let recorder = RecordingSink::new();
            let mut transport = Transport::new(recorder.clone());
            transport.set_bpm(bpm);
            transport.load_sequence(PolyphonicSequence::new(events, total_ticks));
            transport.set_playing(true);

            for reading in readings {
                transport.advance_to(reading);
            }
            transport.set_playing(false);

            prop_assert_eq!(transport.sounding_notes().count(), 0);

            let ringing = still_ringing(&recorder);
            prop_assert!(
                ringing.is_empty(),
                "notes left ringing after stop: {:?}",
                ringing
            );
        }

        /// The play head always stays inside the sequence, so `current_step`
        /// can be used to index a pattern without bounds-checking surprises.
        #[test]
        fn prop_play_head_stays_inside_the_sequence(
            steps in 1usize..17,
            bpm in 1.0f64..400.0,
            readings in proptest::collection::vec(0u64..5_000_000, 1..40),
        ) {
            let pitches: Vec<u8> =
                (0..steps).map(|i| 60 + u8::try_from(i).unwrap()).collect();
            let recorder = RecordingSink::new();
            let mut transport = Transport::new(recorder);
            transport.set_bpm(bpm);
            transport.load_sequence(sequence(&pitches));
            transport.set_playing(true);

            for reading in readings {
                transport.advance_to(reading);
                prop_assert!(
                    transport.current_step() < steps,
                    "step {} outside a {}-step sequence",
                    transport.current_step(),
                    steps
                );
            }
        }
    }
}

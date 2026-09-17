pub mod state;

use crate::{
    playback::state::{
        MidiEventType, PolyphonicSequence, SharedState, TimedEvent,
        TICKS_PER_STEP,
    },
    MixerState, Note, Sequence,
};
use log::{debug, error, info, warn};
use num::integer;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Random velocity spread applied to every emitted note, so repeated steps do
/// not sound mechanical.
const VELOCITY_JITTER: i16 = 8;

/// Voices that crossfade below this velocity are dropped entirely, so a ratio
/// at either extreme really does silence the opposite sequencer.
const MIN_AUDIBLE_VELOCITY: u8 = 8;

pub struct Mixer {
    state: MixerState,
    sequences: (Sequence, Sequence),
    shared_state: Arc<RwLock<SharedState>>,
    rx_sequence: mpsc::Receiver<(Option<Sequence>, Option<Sequence>)>,
    tx_polyphonic_sequence: mpsc::Sender<PolyphonicSequence>,
}

impl Mixer {
    pub fn new(
        shared_state: Arc<RwLock<SharedState>>,
        rx_sequence: mpsc::Receiver<(Option<Sequence>, Option<Sequence>)>,
        tx_polyphonic_sequence: mpsc::Sender<PolyphonicSequence>,
    ) -> Self {
        Mixer {
            state: MixerState::default(),
            sequences: (Sequence::default(), Sequence::default()),
            shared_state,
            rx_sequence,
            tx_polyphonic_sequence,
        }
    }

    pub async fn run(&mut self) {
        loop {
            let r_state = self.shared_state.read().await.mixer;
            if r_state != self.state {
                debug!("Mixer received update request");
                self.state = r_state;
                self.publish().await;
            }

            while let Ok(sequences) = self.rx_sequence.try_recv() {
                debug!("Mixer received sequences.");
                match sequences {
                    (Some(left), Some(right)) => self.sequences = (left, right),
                    (Some(left), None) => {
                        self.sequences = (left, self.sequences.1.clone())
                    }
                    (None, Some(right)) => {
                        self.sequences = (self.sequences.0.clone(), right)
                    }
                    (None, None) => {}
                }
                self.publish().await;
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }

    async fn publish(&mut self) {
        let mixed_sequence = self.mix();
        if let Err(e) = self.tx_polyphonic_sequence.send(mixed_sequence).await {
            error!("Error sending mixed sequence: {e}");
        }
    }

    /// Fold the two source sequences into one polyphonic sequence.
    ///
    /// The mixer ratio is an equal-power crossfade between the two sequencers:
    /// `0.0` is fully left, `1.0` is fully right, `0.5` sounds both.
    #[must_use]
    pub fn mix(&self) -> PolyphonicSequence {
        let len_a = self.sequences.0.notes.len();
        let len_b = self.sequences.1.notes.len();

        if len_a == 0 || len_b == 0 {
            warn!(
                "Mixer received an empty sequence (lengths {len_a} and {len_b}); emitting silence"
            );
            return PolyphonicSequence::default();
        }

        // lcm already equals max() whenever one length divides the other, so
        // the two cases need no separate handling.
        let sequence_length = integer::lcm(len_a, len_b);
        let total_ticks = sequence_length as u32 * TICKS_PER_STEP;

        let (gain_a, gain_b) = crossfade_gains(self.state.ratio);

        let mut timed_events: Vec<TimedEvent> = Vec::new();
        for i in 0..sequence_length {
            let tick = i as u32 * TICKS_PER_STEP;

            let voice_a = voice(self.sequences.0.notes[i % len_a], gain_a);
            let voice_b = voice(self.sequences.1.notes[i % len_b], gain_b);

            match (voice_a, voice_b) {
                (Some((a, vel_a)), Some((b, vel_b))) if a.pitch == b.pitch => {
                    // Both sequencers landed on the same pitch. Emit one note
                    // at the louder velocity rather than two overlapping
                    // Note-Ons that a synth would have to disambiguate.
                    let note = if a.duration.steps() >= b.duration.steps() {
                        a
                    } else {
                        b
                    };
                    push_note(
                        note,
                        vel_a.max(vel_b),
                        tick,
                        total_ticks,
                        &mut timed_events,
                    );
                }
                (voice_a, voice_b) => {
                    for (note, velocity) in
                        voice_a.into_iter().chain(voice_b.into_iter())
                    {
                        push_note(
                            note,
                            velocity,
                            tick,
                            total_ticks,
                            &mut timed_events,
                        );
                    }
                }
            }
        }

        info!(
            "Created sequence of {sequence_length} steps ({} events) from sequences of length {len_a} and {len_b}",
            timed_events.len()
        );

        PolyphonicSequence::new(timed_events, total_ticks)
    }
}

/// Equal-power crossfade gains for `(left, right)`.
///
/// Equal-power rather than linear so that a note does not audibly dip when the
/// ratio is near the centre.
fn crossfade_gains(ratio: f32) -> (f32, f32) {
    let ratio = ratio.clamp(0.0, 1.0);
    ((1.0 - ratio).sqrt(), ratio.sqrt())
}

/// Apply the crossfade gain to a note, returning `None` for rests and for
/// voices that faded below audibility.
fn voice(note: Note, gain: f32) -> Option<(Note, u8)> {
    if note.is_rest() {
        return None;
    }

    // Decide audibility from the crossfade alone, before jitter, so that a
    // fully crossfaded-out voice can never be nudged back into audibility.
    let base = (f32::from(note.velocity) * gain).round().clamp(0.0, 127.0);
    if base < f32::from(MIN_AUDIBLE_VELOCITY) {
        return None;
    }

    let jitter = rand::random_range(-VELOCITY_JITTER..=VELOCITY_JITTER);
    let velocity = (base as i16 + jitter).clamp(1, 127) as u8;
    Some((note, velocity))
}

/// Emit the Note-On/Note-Off pair for one voice.
///
/// The gate ends one tick short of the note's nominal length so a repeat of the
/// same pitch on the next step is not swallowed by this note's release, and is
/// clamped inside the sequence so the release always fires before the loop
/// point (the engine restarts its event cursor when the sequence wraps).
fn push_note(
    note: Note,
    velocity: u8,
    tick: u32,
    total_ticks: u32,
    timed_events: &mut Vec<TimedEvent>,
) {
    let gate = (note.duration.steps() * TICKS_PER_STEP).saturating_sub(1);
    let off_tick = (tick + gate).min(total_ticks.saturating_sub(1));

    timed_events.push(TimedEvent {
        tick,
        event: MidiEventType::NoteOn {
            pitch: note.pitch,
            velocity,
        },
    });
    timed_events.push(TimedEvent {
        tick: off_tick,
        event: MidiEventType::NoteOff { pitch: note.pitch },
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoteDuration;
    use std::collections::HashMap;

    /// Build a sequence from pitches; `0` is a rest.
    fn sequence(pitches: &[u8]) -> Sequence {
        Sequence {
            notes: pitches
                .iter()
                .map(|&pitch| {
                    if pitch == 0 {
                        Note::rest()
                    } else {
                        Note::new(pitch, 100, NoteDuration::Sixteenth)
                    }
                })
                .collect(),
        }
    }

    fn mixer_with(a: Sequence, b: Sequence, ratio: f32) -> Mixer {
        let (_tx_seq, rx_seq) = mpsc::channel(1);
        let (tx_poly, _rx_poly) = mpsc::channel(1);
        let mut mixer = Mixer::new(
            Arc::new(RwLock::new(SharedState::default())),
            rx_seq,
            tx_poly,
        );
        mixer.sequences = (a, b);
        mixer.state = MixerState { ratio };
        mixer
    }

    fn pitches_in(mixed: &PolyphonicSequence) -> Vec<u8> {
        let mut pitches: Vec<u8> = mixed
            .events()
            .iter()
            .filter_map(|e| match e.event {
                MidiEventType::NoteOn { pitch, .. } => Some(pitch),
                MidiEventType::NoteOff { .. } => None,
            })
            .collect();
        pitches.sort_unstable();
        pitches.dedup();
        pitches
    }

    /// Every Note-On must be released. The gate offset used to be applied twice
    /// — once at the call site and once inside the push helper — which pushed
    /// the final Note-Off past the loop point, where the engine never reached
    /// it.
    #[test]
    fn test_note_ons_and_offs_balance() {
        let mixed =
            mixer_with(sequence(&[60, 0, 62, 0]), sequence(&[67, 67, 0]), 0.5)
                .mix();

        let mut held: HashMap<u8, i32> = HashMap::new();
        for event in mixed.events() {
            match event.event {
                MidiEventType::NoteOn { pitch, .. } => {
                    *held.entry(pitch).or_default() += 1
                }
                MidiEventType::NoteOff { pitch } => {
                    *held.entry(pitch).or_default() -= 1
                }
            }
        }

        assert!(!mixed.is_empty(), "expected some events");
        assert!(
            held.values().all(|&count| count == 0),
            "unbalanced Note-On/Note-Off counts: {held:?}"
        );
    }

    /// The engine restarts its event cursor when the sequence wraps, so any
    /// event at or past `total_ticks` is unreachable and its note hangs.
    #[test]
    fn test_every_event_falls_inside_the_sequence() {
        let mixed =
            mixer_with(sequence(&[60, 62, 64, 65]), sequence(&[67, 0, 0]), 0.5)
                .mix();

        for event in mixed.events() {
            assert!(
                event.tick < mixed.total_ticks(),
                "event at tick {} is outside a {}-tick sequence",
                event.tick,
                mixed.total_ticks()
            );
        }
    }

    /// The engine stops dispatching at the first event in the future, so an
    /// out-of-order list stalls playback rather than merely reordering it.
    #[test]
    fn test_events_are_sorted_by_tick() {
        let mixed =
            mixer_with(sequence(&[60, 0, 62, 0]), sequence(&[67, 69]), 0.5)
                .mix();

        assert!(
            mixed.events().windows(2).all(|w| w[0].tick <= w[1].tick),
            "events are not sorted: {:?}",
            mixed.events()
        );
    }

    /// A note must release before the next step begins, or a repeat of the same
    /// pitch is swallowed by its predecessor's release.
    #[test]
    fn test_gate_ends_before_the_next_step() {
        let mixed =
            mixer_with(sequence(&[60, 60]), sequence(&[0, 0]), 0.0).mix();

        let ons: Vec<u32> = mixed
            .events()
            .iter()
            .filter(|e| matches!(e.event, MidiEventType::NoteOn { .. }))
            .map(|e| e.tick)
            .collect();
        let offs: Vec<u32> = mixed
            .events()
            .iter()
            .filter(|e| matches!(e.event, MidiEventType::NoteOff { .. }))
            .map(|e| e.tick)
            .collect();

        assert_eq!(ons, vec![0, TICKS_PER_STEP]);
        assert_eq!(offs, vec![TICKS_PER_STEP - 1, 2 * TICKS_PER_STEP - 1]);
    }

    /// `num::abs_sub` is the *positive* difference, not the absolute one, so
    /// ratios at or above 0.7 used to drop coincident notes entirely.
    #[test]
    fn test_no_ratio_produces_silence() {
        for ratio in [0.0, 0.1, 0.25, 0.5, 0.7, 0.75, 0.9, 1.0] {
            let mixed =
                mixer_with(sequence(&[60, 60]), sequence(&[67, 67]), ratio)
                    .mix();
            assert!(
                !mixed.is_empty(),
                "mixer ratio {ratio} produced silence where both sequencers play"
            );
        }
    }

    /// The ratio is a crossfade: each extreme should isolate one sequencer.
    #[test]
    fn test_ratio_extremes_isolate_one_sequencer() {
        let left = sequence(&[60, 60]);
        let right = sequence(&[67, 67]);

        assert_eq!(
            pitches_in(&mixer_with(left.clone(), right.clone(), 0.0).mix()),
            vec![60],
            "ratio 0.0 should play only the left sequencer"
        );
        assert_eq!(
            pitches_in(&mixer_with(left.clone(), right.clone(), 1.0).mix()),
            vec![67],
            "ratio 1.0 should play only the right sequencer"
        );
        assert_eq!(
            pitches_in(&mixer_with(left, right, 0.5).mix()),
            vec![60, 67],
            "ratio 0.5 should play both sequencers"
        );
    }

    /// Two sequencers landing on the same pitch should produce one note, not
    /// two overlapping Note-Ons a synth has to disambiguate.
    #[test]
    fn test_identical_pitches_collapse_to_one_note() {
        let mixed =
            mixer_with(sequence(&[60, 60]), sequence(&[60, 60]), 0.5).mix();

        assert_eq!(mixed.events().len(), 4, "expected one note pair per step");
    }

    #[test]
    fn test_empty_sequence_is_silent_rather_than_panicking() {
        let mixed = mixer_with(Sequence::empty(), sequence(&[60]), 0.5).mix();
        assert!(mixed.is_empty());
    }
}

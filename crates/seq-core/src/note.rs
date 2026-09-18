//! The pattern types every generator produces and the mixer consumes.
//!
//! A [`Pattern`] is a fixed-capacity, `Copy` value — no allocation, a known
//! size (~1.5 KB), and cheap to hand between tasks or into an interrupt
//! handler. A rest is the *absence* of a [`Voice`] (`None`), not a sentinel
//! pitch, so MIDI note 0 remains a playable note.

use core::fmt;

use crate::event::TICKS_PER_STEP;

/// Longest pattern a generator can produce, in steps.
pub const MAX_STEPS: usize = 64;

/// Most simultaneous voices a single pattern step can hold.
pub const MAX_VOICES: usize = 4;

/// Note durations in musical notation. The discriminants are lengths in
/// sixteenth notes, which is also the sequencer's step resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoteDuration {
    Sixteenth = 1,
    Eighth = 2,
    DottedEight = 3,
    Quarter = 4,
    DottedQuarter = 6,
    Half = 8,
    DottedHalf = 12,
    Whole = 16,
}

impl NoteDuration {
    /// Length of this duration in sequencer steps (sixteenth notes).
    #[must_use]
    pub fn steps(self) -> u32 {
        self as u32
    }

    /// Length of this duration in ticks.
    #[must_use]
    pub fn ticks(self) -> u32 {
        self.steps() * TICKS_PER_STEP
    }
}

/// One sounding note within a step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Voice {
    pub pitch: u8,
    pub velocity: u8,
    /// How long the note is held, in ticks. Generators conventionally end the
    /// gate one tick short of the note's nominal length so a repeat of the
    /// same pitch is not swallowed by this note's release.
    pub gate_ticks: u16,
}

/// One step of a pattern: up to [`MAX_VOICES`] simultaneous voices.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Step {
    voices: [Option<Voice>; MAX_VOICES],
}

impl Step {
    /// A silent step.
    #[must_use]
    pub fn rest() -> Self {
        Self::default()
    }

    /// A step sounding exactly one voice.
    #[must_use]
    pub fn single(voice: Voice) -> Self {
        let mut step = Self::default();
        step.voices[0] = Some(voice);
        step
    }

    /// Add a voice; returns it back if the step is already full.
    pub fn add_voice(&mut self, voice: Voice) -> Result<(), Voice> {
        match self.voices.iter_mut().find(|slot| slot.is_none()) {
            Some(slot) => {
                *slot = Some(voice);
                Ok(())
            }
            None => Err(voice),
        }
    }

    /// The voices sounding on this step.
    pub fn voices(&self) -> impl Iterator<Item = &Voice> {
        self.voices.iter().flatten()
    }

    #[must_use]
    pub fn is_rest(&self) -> bool {
        self.voices.iter().all(Option::is_none)
    }
}

/// A fixed-capacity musical pattern: up to [`MAX_STEPS`] steps of up to
/// [`MAX_VOICES`] voices each.
///
/// `Copy` on purpose — a pattern is a value, ~1.5 KB, and passing it by value
/// is what lets sources, mixers and transports exchange them without an
/// allocator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Pattern {
    steps: [Step; MAX_STEPS],
    len: u16,
}

impl Pattern {
    /// An all-rest pattern of `len` steps. `len` is clamped to
    /// `1..=MAX_STEPS`.
    #[must_use]
    pub fn new(len: usize) -> Self {
        let len = len.clamp(1, MAX_STEPS);
        Self {
            steps: [Step::rest(); MAX_STEPS],
            // Infallible: clamped to MAX_STEPS (64) above.
            len: u16::try_from(len).unwrap_or(1),
        }
    }

    /// Number of active steps.
    #[must_use]
    pub fn len(&self) -> usize {
        usize::from(self.len)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The active steps.
    #[must_use]
    pub fn steps(&self) -> &[Step] {
        &self.steps[..self.len()]
    }

    /// Replace one step. Indices past the end are ignored.
    pub fn set_step(&mut self, index: usize, step: Step) {
        if index < self.len() {
            self.steps[index] = step;
        }
    }

    /// Whether step `index` strikes at least one voice. `false` past the end.
    #[must_use]
    pub fn hit(&self, index: usize) -> bool {
        self.steps().get(index).is_some_and(|step| !step.is_rest())
    }
}

/// Sixteen rest steps — one bar of sixteenths, silent.
impl Default for Pattern {
    fn default() -> Self {
        Self::new(16)
    }
}

/// A note name plus octave, formatted like `C4` or `D#1 / Eb1`.
///
/// Returned instead of a `String` so the naming logic stays `no_std`; call
/// `.to_string()` (or format it) on hosts that want owned text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NoteName {
    natural: &'static str,
    flat: Option<&'static str>,
    octave: i16,
}

impl fmt::Display for NoteName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.natural, self.octave)?;
        if let Some(flat) = self.flat {
            write!(f, " / {}{}", flat, self.octave)?;
        }
        Ok(())
    }
}

/// The conventional name of a MIDI pitch, on the scale where 60 is C4.
#[must_use]
pub fn note_name(pitch: u8) -> NoteName {
    const NAMES: [(&str, Option<&str>); 12] = [
        ("C", None),
        ("C#", Some("Db")),
        ("D", None),
        ("D#", Some("Eb")),
        ("E", None),
        ("F", None),
        ("F#", Some("Gb")),
        ("G", None),
        ("G#", Some("Ab")),
        ("A", None),
        ("A#", Some("Bb")),
        ("B", None),
    ];
    let (natural, flat) = NAMES[usize::from(pitch % 12)];
    // MIDI note 12 is C0, so note 0 lands in octave -1. Computed in i16 so the
    // rest of the arithmetic cannot underflow for pitches below C0.
    let octave = i16::from(pitch) / 12 - 1;
    NoteName {
        natural,
        flat,
        octave,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pitch_to_note() {
        assert_eq!(note_name(21).to_string(), "A0");
        assert_eq!(note_name(26).to_string(), "D1");
        assert_eq!(note_name(27).to_string(), "D#1 / Eb1");
        assert_eq!(note_name(33).to_string(), "A1");
        assert_eq!(note_name(60).to_string(), "C4");
        assert_eq!(note_name(69).to_string(), "A4");
        assert_eq!(note_name(96).to_string(), "C7");
        assert_eq!(note_name(127).to_string(), "G9");
    }

    /// Notes below C0 used to underflow `pitch - 12` and panic in the old
    /// `String`-based implementation; keep the whole range covered.
    #[test]
    fn test_pitch_to_note_below_c0() {
        assert_eq!(note_name(0).to_string(), "C-1");
        assert_eq!(note_name(11).to_string(), "B-1");
        assert_eq!(note_name(12).to_string(), "C0");
    }

    #[test]
    fn test_pitch_to_note_never_panics() {
        for pitch in 0..=u8::MAX {
            let _ = note_name(pitch).to_string();
        }
    }

    #[test]
    fn test_pattern_len_is_clamped() {
        assert_eq!(Pattern::new(0).len(), 1);
        assert_eq!(Pattern::new(16).len(), 16);
        assert_eq!(Pattern::new(1000).len(), MAX_STEPS);
    }

    #[test]
    fn test_step_voice_capacity() {
        let voice = Voice {
            pitch: 60,
            velocity: 100,
            gate_ticks: 119,
        };

        let mut step = Step::rest();
        assert!(step.is_rest());

        for _ in 0..MAX_VOICES {
            step.add_voice(voice).unwrap();
        }
        assert_eq!(step.add_voice(voice), Err(voice), "step should be full");
        assert_eq!(step.voices().count(), MAX_VOICES);
    }

    #[test]
    fn test_pattern_hit_is_false_past_the_end() {
        let mut pattern = Pattern::new(4);
        pattern.set_step(
            0,
            Step::single(Voice {
                pitch: 60,
                velocity: 100,
                gate_ticks: 119,
            }),
        );

        assert!(pattern.hit(0));
        assert!(!pattern.hit(1));
        assert!(!pattern.hit(4), "past the end must read as a rest");
        assert!(!pattern.hit(usize::MAX));
    }
}

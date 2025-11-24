pub mod euclidean;

use std::fmt::Debug;

pub trait Sequencer {
    fn generate_sequence(&self) -> Sequence;
    fn run(&mut self) -> impl std::future::Future<Output = ()> + Send;
}

/// NoteDuration is a helper enum to define note durations in musical notation. These durations are
/// then converted to seconds in playback.
#[derive(Clone, Copy, Debug)]
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

/// A Note is a MIDI object with pitch, velocity, duration, and a channel.
#[derive(Clone, Copy, Debug)]
pub struct Note {
    pub pitch: u8,
    pub velocity: u8,
    pub duration: NoteDuration,
}

impl Note {
    pub fn new(pitch: u8, velocity: u8, duration: NoteDuration) -> Self {
        Note {
            pitch,
            velocity,
            duration,
        }
    }
}

/// A Sequence is defined as a vector of Notes, produced by one single Sequencer.
#[derive(Clone)]
pub struct Sequence {
    pub notes: Vec<Note>,
}

impl Sequence {
    pub fn empty() -> Self {
        Sequence { notes: vec![] }
    }

    pub fn midi_to_note_name(pitch: u8) -> String {
        let note_names = [
            "C.",
            "C#. / Db.",
            "D.",
            "D#. / Eb.",
            "E.",
            "F.",
            "F#. / Gb.",
            "G.",
            "G#. / Ab.",
            "A.",
            "A#. / Bb.",
            "B.",
        ];
        let octave = ((pitch - 12) as f32 / 12.).floor();
        let note = note_names[((pitch - 12) % 12) as usize];

        note.replace(".", &format!("{octave}"))
    }
}

impl Default for Sequence {
    fn default() -> Self {
        let notes = vec![Note::new(0, 0, NoteDuration::Sixteenth); 16];
        Sequence { notes }
    }
}

/// A MixedSequence is the result of mixing two Sequences in the Mixer.
#[derive(Debug)]
pub struct MixedSequence {
    pub notes: Vec<(Option<Note>, Option<Note>)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pitch_to_note() {
        assert_eq!(Sequence::midi_to_note_name(21), "A0");
        assert_eq!(Sequence::midi_to_note_name(26), "D1");
        assert_eq!(Sequence::midi_to_note_name(27), "D#1 / Eb1");
        assert_eq!(Sequence::midi_to_note_name(33), "A1");
        assert_eq!(Sequence::midi_to_note_name(60), "C4");
        assert_eq!(Sequence::midi_to_note_name(69), "A4");
        assert_eq!(Sequence::midi_to_note_name(96), "C7");
        assert_eq!(Sequence::midi_to_note_name(127), "G9");
    }
}

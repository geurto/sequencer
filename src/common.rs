use std::fmt::{Debug, Formatter};

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

#[derive(Clone, Debug)]
pub struct Note {
    pub pitch: u8,
    pub velocity: u8,
    pub duration: f32,
}

impl Note {
    pub fn new(pitch: u8, velocity: u8, note_duration: NoteDuration, bpm: f32) -> Self {
        let duration = match note_duration {
            NoteDuration::Sixteenth => 60000.0 / bpm / 4.0,
            NoteDuration::Eighth => 60000.0 / bpm / 2.0,
            NoteDuration::DottedEight => 60000.0 / bpm / 2.0 * 1.5,
            NoteDuration::Quarter => 60000.0 / bpm,
            NoteDuration::DottedQuarter => 60000.0 / bpm * 1.5,
            NoteDuration::Half => 60000.0 / bpm * 2.0,
            NoteDuration::DottedHalf => 60000.0 / bpm * 2.0 * 1.5,
            NoteDuration::Whole => 60000.0 / bpm * 4.0,
        };

        Note {
            pitch,
            velocity,
            duration,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Sequence {
    pub notes: Vec<Note>,
}

impl Sequence {
    pub fn default() -> Self {
        let mut notes = vec![];
        notes.push(Note::new(0, 0, NoteDuration::Whole, 120.0));
        Sequence { notes }
    }

    pub fn empty() -> Self {
        Sequence { notes: vec![] }
    }

    pub fn clear(&mut self) {
        self.notes.clear();
    }
}

pub struct SharedState {
    pub bpm: f32,
    pub sequence: Sequence,
    pub clock_ticks: u32,
    pub quarter_notes: u32,
}

impl SharedState {
    pub fn new(bpm: f32) -> Self {
        SharedState {
            bpm,
            sequence: Sequence::default(),
            clock_ticks: 0,
            quarter_notes: 0,
        }
    }

    fn midi_to_note_name(pitch: u8) -> String {
        let note_names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
        let octave = (pitch / 12) as i8 - 1;
        let note = note_names[(pitch % 12) as usize];
        format!("{}{}", note, octave)
    }

    fn duration_to_symbol(duration: f32, bpm: f32) -> String {
        let sixteenth_note_duration = 60000.0 / bpm / 4.0;
        let num_sixteenth_notes = (duration / sixteenth_note_duration).round() as usize;
        "-".repeat(num_sixteenth_notes)
    }
}

impl Debug for SharedState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut result = String::new();
        result.push_str(&format!("BPM: {} | Sequence: ", self.bpm));

        for note in &self.sequence.notes {
            let note_name = if note.pitch == 0 {
                " ".to_string()
            } else {
                SharedState::midi_to_note_name(note.pitch)
            };
            let duration_symbol = SharedState::duration_to_symbol(note.duration, self.bpm);
            result.push_str(&format!("{}{}, ", note_name, duration_symbol));
        }

        result.fmt(f)
    }
}

pub enum Input {
    Bpm(f32),
    Sequence(Sequence),
}
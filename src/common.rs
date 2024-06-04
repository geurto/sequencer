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
}

pub struct SharedState {
    pub bpm: f32,
    pub sequence: Sequence,
}

pub enum Input {
    Bpm(f32),
    Sequence(Sequence),
}
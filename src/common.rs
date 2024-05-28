#[derive(Clone, Debug)]
pub struct Sequence {
    pub pitch: Vec<u8>,
    pub velocity: Vec<u8>,
    pub duration: Vec<u16>,
}

pub struct PlaybackState {
    pub bpm: f32,
    pub sequence: Sequence,
}

pub enum Input {
    Bpm(f32),
    Sequence(Sequence),
}
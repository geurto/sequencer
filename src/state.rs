use crate::sequencers::euclidean::config::EuclideanSequencerConfig;
use crate::sequencers::mixer::config::MixerConfig;
use tokio::sync::mpsc;

pub struct SharedState {
    pub playing: bool,
    pub bpm: f32,
    pub midi_channel: u8,
    pub mixer_config: MixerConfig,
    pub clock_ticks: u32,
    pub quarter_notes: u32,
}

impl SharedState {
    pub fn new(bpm: f32) -> Self {
        SharedState {
            playing: false,
            bpm,
            midi_channel: 0,
            mixer_config: MixerConfig::new(),
            clock_ticks: 0,
            quarter_notes: 0,
        }
    }

    pub fn increase_bpm(&mut self) {
        self.bpm += 1.0;
    }

    pub fn decrease_bpm(&mut self) {
        self.bpm -= 1.0;
    }

    pub fn change_midi_channel(&mut self) {
        self.midi_channel = (self.midi_channel + 1) % 16;
    }
}

pub struct SequencerChannels {
    pub a_tx: mpsc::Sender<EuclideanSequencerConfig>,
    pub b_tx: mpsc::Sender<EuclideanSequencerConfig>,
    pub mixer_tx: mpsc::Sender<()>,
}

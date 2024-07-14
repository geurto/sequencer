use tokio::sync::mpsc;
use crate::note::Sequence;
use crate::sequencers::euclidean::config::EuclideanSequencerConfig;
use crate::sequencers::markov::config::MarkovSequencerConfig;
use crate::sequencers::mixer::config::MixerConfig;

pub struct SharedState {
    pub playing: bool,
    pub bpm: f32,
    pub mixer_config: MixerConfig,
    pub clock_ticks: u32,
    pub quarter_notes: u32,
}

impl SharedState {
    pub fn new(bpm: f32) -> Self {
        SharedState {
            playing: false,
            bpm,
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
}

pub struct SequencerChannels {
    pub euclidean_tx: mpsc::Sender<EuclideanSequencerConfig>,
    pub markov_tx: mpsc::Sender<MarkovSequencerConfig>,
    pub mixer_tx: mpsc::Sender<MixerConfig>,
}

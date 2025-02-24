use anyhow::Result;
use device_query::{DeviceQuery, DeviceState, Keycode};
use log::info;
use std::collections::HashSet;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};

use crate::sequencers::euclidean::state::{EuclideanSequencerInput, EuclideanSequencerState};
use crate::sequencers::mixer::state::{MixerInput, MixerState};
use crate::state::{ActiveSequencer, SharedState};

pub enum Input {
    Bpm(f32),
    TogglePlayback,
    ChangeMidiChannel,
    IncreaseBpm,
    DecreaseBpm,
    Euclidean(EuclideanSequencerInput),
    Mixer(MixerInput),
}

pub struct InputHandler {
    state: RwLock<SharedState>,
    tx_left: mpsc::Sender<EuclideanSequencerState>,
    tx_right: mpsc::Sender<EuclideanSequencerState>,
    tx_mixer: mpsc::Sender<MixerState>,
}

impl InputHandler {
    pub fn new(
        state: RwLock<SharedState>,
        tx_left: mpsc::Sender<EuclideanSequencerState>,
        tx_right: mpsc::Sender<EuclideanSequencerState>,
        tx_mixer: mpsc::Sender<MixerState>,
    ) -> Self {
        Self {
            state,
            tx_left,
            tx_right,
            tx_mixer,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        let device_state = DeviceState::new();
        let mut last_keys = HashSet::new();

        loop {
            let keys: HashSet<Keycode> = device_state.get_keys().into_iter().collect();

            let diff = keys.difference(&last_keys);

            if diff.clone().next().is_some() {
                let mut w_state = self.state.write().await;
                let mut sequencer_config = match w_state.active_sequencer {
                    ActiveSequencer::Left => w_state.left_state.clone(),
                    ActiveSequencer::Right => w_state.right_state.clone(),
                };
                for key in diff {
                    match key {
                        Keycode::Space => w_state.playing = !w_state.playing,
                        Keycode::C => {
                            w_state.change_midi_channel();
                            info!("Changing MIDI channel to {}", w_state.midi_channel + 1)
                        }
                        Keycode::R => w_state.mixer_state.increase_ratio(),
                        Keycode::F => w_state.mixer_state.decrease_ratio(),
                        Keycode::Up => sequencer_config.increase_steps(),
                        Keycode::Down => sequencer_config.decrease_steps(),
                        Keycode::Right => sequencer_config.increase_pulses(),
                        Keycode::Left => sequencer_config.decrease_pulses(),
                        Keycode::W => sequencer_config.change_pitch(1),
                        Keycode::S => sequencer_config.change_pitch(-1),
                        Keycode::D => sequencer_config.change_pitch(12),
                        Keycode::A => sequencer_config.change_pitch(-12),
                        _ => {}
                    };
                }

                self.tx_left.send(w_state.left_state.clone()).await?;
                self.tx_right.send(w_state.right_state.clone()).await?;
                self.tx_mixer.send(w_state.mixer_state.clone()).await?;
            }

            last_keys = keys;
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
}

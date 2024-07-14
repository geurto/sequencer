use device_query::{DeviceState, DeviceQuery, Keycode};
use log::info;
use std::collections::HashSet;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};

use crate::sequencers::euclidean::config::EuclideanSequencerConfig;
use crate::sequencers::euclidean::input::EuclideanSequencerInput;
use crate::sequencers::markov::config::MarkovSequencerConfig;
use crate::sequencers::markov::input::MarkovSequencerInput;
use crate::sequencers::mixer::config::MixerConfig;
use crate::sequencers::mixer::input::MixerInput;
use crate::state::{SequencerChannels, SharedState};

pub enum Input {
    Bpm(f32),
    TogglePlayback,
    IncreaseBpm,
    DecreaseBpm,
    Euclidean(EuclideanSequencerInput),
    Markov(MarkovSequencerInput),
    Mixer(MixerInput),
}

pub fn spawn_input_handler(
    tx: mpsc::Sender<Input>) {
    thread::spawn(move || {
        let device_state = DeviceState::new();
        let mut last_keys = HashSet::new();

        loop {
            let keys: HashSet<Keycode> = device_state.get_keys().into_iter().collect();

            for key in keys.difference(&last_keys) {
                let input = match key {
                    Keycode::Space => Some(Input::TogglePlayback),
                    Keycode::R => Some(Input::Mixer(MixerInput::IncreaseRatio)),
                    Keycode::F => Some(Input::Mixer(MixerInput::DecreaseRatio)),
                    Keycode::Up => Some(Input::Euclidean(EuclideanSequencerInput::IncreaseSteps)),
                    Keycode::Down => Some(Input::Euclidean(EuclideanSequencerInput::DecreaseSteps)),
                    Keycode::Left => Some(Input::Euclidean(EuclideanSequencerInput::DecreasePulses)),
                    Keycode::Right => Some(Input::Euclidean(EuclideanSequencerInput::IncreasePulses)),
                    Keycode::W => Some(Input::Euclidean(EuclideanSequencerInput::IncreasePitch)),
                    Keycode::S => Some(Input::Euclidean(EuclideanSequencerInput::DecreasePitch)),
                    Keycode::A => Some(Input::Euclidean(EuclideanSequencerInput::DecreaseOctave)),
                    Keycode::D => Some(Input::Euclidean(EuclideanSequencerInput::IncreaseOctave)),
                    _ => None,
                };

                if let Some(input) = input {
                    tx.blocking_send(input).unwrap();
                }
            }

            last_keys = keys;
            thread::sleep(Duration::from_millis(10));
        }
    });
}

pub async fn process_input(
    mut rx: mpsc::Receiver<Input>,
    shared_state: Arc<Mutex<SharedState>>,
    sequencer_channels: SequencerChannels,
) {
    let mut euclidean_config = EuclideanSequencerConfig::new();
    let markov_config = MarkovSequencerConfig::new();
    loop {
        if let Some(input) = rx.recv().await {
            let mut state = shared_state.lock().await;
            match input {
                Input::Bpm(bpm) => {
                    info!("Changing BPM to {}", bpm);
                    state.bpm = bpm;
                },
                Input::TogglePlayback => {
                    info!("Toggling playback");
                    state.playing = !state.playing;
                },
                Input::IncreaseBpm => state.increase_bpm(),
                Input::DecreaseBpm => state.decrease_bpm(),
                Input::Euclidean(euclidean_input) => {
                    match euclidean_input {
                        EuclideanSequencerInput::IncreaseSteps => euclidean_config.increase_steps(),
                        EuclideanSequencerInput::DecreaseSteps => euclidean_config.decrease_steps(),
                        EuclideanSequencerInput::IncreasePulses => euclidean_config.increase_pulses(),
                        EuclideanSequencerInput::DecreasePulses => euclidean_config.decrease_pulses(),
                        EuclideanSequencerInput::IncreasePhase => euclidean_config.increase_phase(),
                        EuclideanSequencerInput::DecreasePhase => euclidean_config.decrease_phase(),
                        EuclideanSequencerInput::IncreasePitch => euclidean_config.change_pitch(1),
                        EuclideanSequencerInput::DecreasePitch => euclidean_config.change_pitch(-1),
                        EuclideanSequencerInput::IncreaseOctave => euclidean_config.change_pitch(12),
                        EuclideanSequencerInput::DecreaseOctave => euclidean_config.change_pitch(-12),
                    }
                    sequencer_channels.euclidean_tx.send(euclidean_config.clone()).await.unwrap();
                },
                Input::Markov(markov_input) => {
                    match markov_input {
                        _ => {}
                    }
                    sequencer_channels.markov_tx.send(markov_config.clone()).await.unwrap();
                },
                Input::Mixer(mixer_input) => {
                    match mixer_input {
                        MixerInput::IncreaseRatio => state.mixer_config.increase_ratio(),
                        MixerInput::DecreaseRatio => state.mixer_config.decrease_ratio(),
                    }
                },
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

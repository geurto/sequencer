use std::thread;
use tokio::sync::mpsc;
use crate::common::Input;
use crate::sequencers::euclidean::EuclideanSequencerInput;
use device_query::{DeviceState, DeviceQuery, Keycode};
use std::collections::HashSet;
use std::time::Duration;

pub fn spawn_input_handler(tx: mpsc::Sender<Input>) {
    thread::spawn(move || {
        let device_state = DeviceState::new();
        let mut last_keys = HashSet::new();

        loop {
            let keys: HashSet<Keycode> = device_state.get_keys().into_iter().collect();

            for key in keys.difference(&last_keys) {
                let input = match key {
                    Keycode::Space => Some(Input::TogglePlayback),
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

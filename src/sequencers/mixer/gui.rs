use iced::{widget::slider, Element, Subscription};
use log::info;

use super::state::MixerState;

#[derive(Debug, Clone)]
pub enum Message {
    FromApp(MixerState),
    RatioChanged(f32),
}

pub struct Gui {
    state: MixerState,
}

impl Gui {
    pub fn new() -> Self {
        Self {
            state: MixerState::new(),
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::FromApp(new_state) => {
                info!("Received new FromApp Mixer state: {:?}", new_state);
                self.state = new_state;
            }
            Message::RatioChanged(ratio) => {
                info!("Received new RatioChanged ratio: {:?}", ratio);
                self.state.ratio = ratio;
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        slider(0.0..=1.0, self.state.ratio, Message::RatioChanged).into()
    }
}

impl Default for Gui {
    fn default() -> Self {
        Self::new()
    }
}

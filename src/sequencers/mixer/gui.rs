use iced::{
    widget::{column, container, slider, text},
    Alignment::Center,
    Element, Length, Subscription,
};
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
                self.state = new_state;
            }
            Message::RatioChanged(ratio) => {
                info!("Received new RatioChanged ratio: {:?}", ratio);
                self.state.ratio = ratio;
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let slider = slider(0.0..=1.0, self.state.ratio, Message::RatioChanged);
        let content = column![
            text!("Mixer"),
            slider,
            text!("[R / F] Increase / Decrease Ratio")
        ]
        .align_x(Center)
        .spacing(20);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Center)
            .align_y(Center)
            .into()
    }
}

impl Default for Gui {
    fn default() -> Self {
        Self::new()
    }
}

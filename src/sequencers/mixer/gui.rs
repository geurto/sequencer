use iced::{
    widget::slider, Alignment::Center, Color, Element, Length, Point, Renderer, Subscription,
};

use log::info;

use super::state::MixerState;

const FILL_COLOR: Color = Color::from_rgb(0.46, 0.23, 0.54);
const BACKGROUND_COLOR: Color = Color::from_rgb(0., 0., 0.);
const CIRCLE_RADIUS: f32 = 20.0;
const CIRCLE_SPACING: f32 = 60.0;

#[derive(Debug, Clone)]
pub enum Message {
    FromApp(MixerState),
}

pub struct Gui {
    state: MixerState,
    index: usize,
}

impl Gui {
    pub fn new(index: usize) -> Self {
        Self {
            state: MixerState::new(),
            index,
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
        }
    }

    pub fn view(&self) -> Element<Message> {
        slider(0.0..=1.0, self.state.ratio, Message::FromApp).into()
    }
}

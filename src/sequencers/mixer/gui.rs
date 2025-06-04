use iced::{
    border::Radius,
    font,
    widget::{
        column, container,
        slider::{self, Handle, Rail, Style},
        text,
    },
    Alignment::Center,
    Element, Font, Length, Subscription, Theme,
};
use log::info;

use crate::gui::CustomTheme;
use crate::sequencers::mixer::MixerState;

#[derive(Debug, Clone)]
pub enum Message {
    FromApp(MixerState),
    RatioChanged(f32),
}

pub struct Gui {
    state: MixerState,
    theme: CustomTheme,
}

impl Gui {
    pub fn new() -> Self {
        Self {
            state: MixerState::new(),
            theme: CustomTheme::default(),
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
        let rail = Rail {
            backgrounds: (
                iced::Background::Color(self.theme.overlay_color),
                iced::Background::Color(self.theme.surface_color),
            ),
            width: 5.,
            border: iced::Border {
                color: self.theme.accent_color_muted,
                width: 2.,
                radius: Radius::default(),
            },
        };
        let handle = Handle {
            shape: slider::HandleShape::Rectangle {
                width: 10,
                border_radius: Radius::default(),
            },
            background: iced::Background::Color(self.theme.overlay_color),
            border_width: 2.,
            border_color: self.theme.accent_color,
        };
        let slider = slider(0.0..=1.0, self.state.ratio, Message::RatioChanged)
            .style(Style { rail, handle });
        let content = column![
            text("Mixer")
                .color(self.theme.primary_text_color)
                .font(self.theme.bold_font),
            slider,
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

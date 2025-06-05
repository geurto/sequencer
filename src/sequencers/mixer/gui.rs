use iced::{
    border::Radius,
    widget::{
        column, container,
        slider::{self, Handle, Rail, Status, Style},
        text,
    },
    Alignment::Center,
    Background, Border, Element, Length, Subscription,
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
        let theme = &self.theme;
        let slider = iced::widget::slider(0.0..=1.0, self.state.ratio, Message::RatioChanged)
            .style(move |_: &iced::Theme, status: Status| {
                let handle_color = match status {
                    Status::Hovered => theme.accent_color,
                    Status::Dragged => theme.primary_color,
                    Status::Active => theme.primary_color_muted,
                };

                let rail_backgrounds = match status {
                    Status::Hovered => (
                        Background::Color(theme.primary_color_muted),
                        Background::Color(theme.surface_color),
                    ),
                    _ => (
                        Background::Color(theme.overlay_color),
                        Background::Color(theme.surface_color),
                    ),
                };

                Style {
                    rail: Rail {
                        backgrounds: rail_backgrounds,
                        width: 5.,
                        border: Border {
                            color: theme.accent_color_muted,
                            width: 2.,
                            radius: Radius::default(),
                        },
                    },
                    handle: Handle {
                        shape: slider::HandleShape::Rectangle {
                            width: 10,
                            border_radius: Radius::default(),
                        },
                        background: Background::Color(handle_color),
                        border_width: 2.,
                        border_color: theme.accent_color,
                    },
                }
            });
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

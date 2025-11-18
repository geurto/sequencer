use super::{Gui, GuiMessage};

use iced::{
    border::Radius,
    widget::slider::{
        self, Handle, Rail, Status as SliderStatus, Style as SliderStyle,
    },
    widget::{column, container, text},
    Alignment::Center,
    Background, Border, Element, Length,
};

impl Gui {
    pub fn view_mixer(&self) -> Element<GuiMessage> {
        let theme = &self.theme;
        let slider = iced::widget::slider(
            0.0..=1.0,
            self.mixer_ratio,
            GuiMessage::MixerRatioChanged,
        )
        .style(move |_: &iced::Theme, status: SliderStatus| {
            let handle_color = match status {
                SliderStatus::Hovered => theme.accent_color,
                SliderStatus::Dragged => theme.primary_color,
                SliderStatus::Active => theme.primary_color_muted,
            };

            let rail_backgrounds = match status {
                SliderStatus::Hovered => (
                    Background::Color(theme.primary_color_muted),
                    Background::Color(theme.surface_color),
                ),
                _ => (
                    Background::Color(theme.overlay_color),
                    Background::Color(theme.surface_color),
                ),
            };

            SliderStyle {
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
                .font(self.theme.bold_font)
                .size(self.theme.header_text_size),
            slider,
        ]
        .align_x(Center)
        .spacing(5);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Center)
            .align_y(Center)
            .into()
    }
}

use super::{Gui, GuiMessage};

use iced::{
    border::Radius,
    widget::{
        button,
        button::{Status as ButtonStatus, Style as ButtonStyle},
        column, container, pick_list, row, text,
    },
    Alignment::Center,
    Background, Border, Element, Length, Shadow,
};

impl Gui {
    pub fn view_midi(&self) -> Element<GuiMessage> {
        let dropdown = pick_list(
            self.midi_out_ports.clone(),
            self.selected_midi_port.clone(),
            GuiMessage::MidiPortSelected,
        )
        .placeholder("Select MIDI output interface");

        let theme = &self.theme;
        let button = button("⟳")
            .on_press(GuiMessage::RefreshMidiPorts)
            .height(25)
            .width(25)
            .style(move |_: &iced::Theme, status: ButtonStatus| {
                let button_color = match status {
                    ButtonStatus::Hovered => theme.accent_color,
                    ButtonStatus::Pressed => theme.primary_color,
                    ButtonStatus::Active => theme.primary_color_muted,
                    ButtonStatus::Disabled => theme.text_color,
                };

                ButtonStyle {
                    background: Some(Background::Color(button_color)),
                    text_color: self.theme.primary_text_color,
                    border: Border {
                        color: button_color,
                        width: 2.,
                        radius: Radius {
                            top_left: 4.,
                            top_right: 4.,
                            bottom_left: 4.,
                            bottom_right: 4.,
                        },
                    },
                    shadow: Shadow::default(),
                }
            });

        let content = column![
            text("MIDI")
                .color(self.theme.primary_text_color)
                .font(self.theme.bold_font)
                .size(self.theme.header_text_size),
            row![dropdown, button].spacing(10)
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

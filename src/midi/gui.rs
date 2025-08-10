use iced::{
    border::Radius,
    widget::{
        button,
        button::{Status, Style},
        column, container, pick_list, row, text,
    },
    Alignment::Center,
    Background, Border, Element, Length, Shadow, Subscription,
};

use tokio::sync::mpsc::Sender;

use crate::gui::CustomTheme;

use super::state::MidiCommand;

#[derive(Clone, Debug)]
pub enum Message {
    RefreshPorts,
    PortsLoaded(Result<Vec<String>, String>),
    PortSelected(String),
    ErrorOccurred(String),
}

pub struct Gui {
    tx: Sender<MidiCommand>,
    midi_out_ports: Vec<String>,
    selected_midi_port: Option<String>,
    theme: CustomTheme,
}

impl Gui {
    pub fn new(tx: Sender<MidiCommand>) -> Self {
        Self {
            tx,
            midi_out_ports: vec!["".to_string()],
            selected_midi_port: None,
            theme: CustomTheme::default(),
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::RefreshPorts => {}
            Message::PortsLoaded(res) => {}
            Message::PortSelected(port) => {}
            Message::ErrorOccurred(err) => {}
        }
    }

    pub fn view(&self) -> Element<Message> {
        let dropdown = pick_list(
            self.midi_out_ports.clone(),
            self.selected_midi_port.clone(),
            Message::PortSelected,
        )
        .placeholder("Select MIDI output interface");

        let theme = &self.theme;
        let button = button("⟳")
            .on_press(Message::RefreshPorts)
            .height(25)
            .width(25)
            .style(move |_: &iced::Theme, status: Status| {
                let button_color = match status {
                    Status::Hovered => theme.accent_color,
                    Status::Pressed => theme.primary_color,
                    Status::Active => theme.primary_color_muted,
                    Status::Disabled => theme.text_color,
                };

                Style {
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
            text("Mixer")
                .color(self.theme.primary_text_color)
                .font(self.theme.bold_font),
            row![dropdown, button].spacing(10)
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

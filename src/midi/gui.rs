use iced::{
    border::Radius,
    widget::{
        button,
        button::{Status, Style},
        column, container, pick_list, row, text,
    },
    Alignment::Center,
    Background, Border, Element, Length, Shadow, Subscription, Task,
};

use log::{info, warn};
use tokio::sync::{mpsc::Sender, oneshot};

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

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::RefreshPorts => {
                info!("Sending GetPorts");
                let tx_midi = self.tx.clone();

                Task::perform(
                    async move {
                        let (tx_oneshot, rx_oneshot) = oneshot::channel();
                        if let Err(e) = tx_midi
                            .send(MidiCommand::GetPorts {
                                responder: tx_oneshot,
                            })
                            .await
                        {
                            warn!("Could not send ports oneshot to GUI: {e}");
                            return Message::ErrorOccurred(format!(
                                "Could not send message to MIDI handler: {e}"
                            ));
                        }

                        match rx_oneshot.await {
                            Ok(ports) => Message::PortsLoaded(Ok(ports)),
                            Err(e) => {
                                warn!("Error receiving ports from oneshot: {e}");
                                Message::PortsLoaded(Err(format!("Oneshot receive error: {e}")))
                            }
                        }
                    },
                    |msg| msg,
                )
            }
            Message::PortsLoaded(result) => {
                match result {
                    Ok(ports) => {
                        self.midi_out_ports = ports;
                        info!("Successfully received new ports: {:?}", self.midi_out_ports);
                    }
                    Err(e) => {
                        warn!("Failed to load ports: {}", e);
                    }
                }
                Task::none()
            }
            _ => Task::none(),
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

pub mod midi;
pub mod mixer;
pub mod sequencers;

use crate::MidiCommand;
use iced::{
    color,
    futures::{channel::mpsc, SinkExt, Stream},
    stream,
    widget::Container,
    widget::{column, container, row, text},
    Alignment::{Center, Start},
    Color, Element, Font, Length, Subscription, Task, Theme,
};
use iced_futures::core::font;
use log::{error, info, warn};
use sequencers::euclidean::{
    Gui as EuclideanGui, Message as EuclideanGuiMessage,
};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc::Sender, oneshot};

#[derive(Debug, Clone)]
pub enum Message {
    ReceivedEvent(Event),
    LeftSequencer(EuclideanGuiMessage),
    RightSequencer(EuclideanGuiMessage),
    MixerRatioChanged(f32),
    NotePlayed(usize),
    RefreshMidiPorts,
    MidiPortsLoaded(Result<Vec<String>, String>),
    MidiPortSelected(String),
    MidiPortSet(String),
    ErrorOccurred(String),
}

pub struct CustomTheme {
    pub primary_color: Color,
    pub primary_color_muted: Color,
    pub secondary_color: Color,
    pub secondary_color_muted: Color,
    pub primary_text_color: Color,
    pub secondary_text_color: Color,
    pub text_color: Color,
    pub surface_color: Color,
    pub overlay_color: Color,
    pub accent_color: Color,
    pub accent_color_muted: Color,
    pub header_font: Font,
    pub bold_font: Font,
    pub header_text_size: u16,
    pub text_size: u16,
}

impl Default for CustomTheme {
    fn default() -> Self {
        // All colors taken from Catppuccin Mocha
        Self {
            primary_color: color!(0xcba6f7),       // Mauve
            primary_color_muted: color!(0x65537b), // Muted mauve

            secondary_color: color!(0xf5c2e7), // Pink
            secondary_color_muted: color!(0x7a6173), // Muted pink

            primary_text_color: color!(0x89b4fa), // Blue
            secondary_text_color: color!(0xb4befe), // Lavender
            text_color: color!(0xcdd6f4),         // Text,

            surface_color: color!(0x1e1e2e), // Base
            overlay_color: color!(0x313244), // Surface0

            accent_color: color!(0xb4befe), // Lavender
            accent_color_muted: color!(0x5a5f7f), // Muted lavender

            header_font: Font {
                weight: font::Weight::Bold,
                stretch: font::Stretch::Expanded,
                ..Font::default()
            },
            bold_font: Font {
                weight: font::Weight::Bold,
                ..Font::default()
            },
            header_text_size: 14,
            text_size: 12,
        }
    }
}

pub struct Gui {
    tx_gui: Arc<Mutex<Option<mpsc::Sender<Message>>>>,
    tx_midi: Sender<MidiCommand>,
    sequencer_left: EuclideanGui,
    sequencer_right: EuclideanGui,
    mixer_ratio: f32,
    current_note_index: usize,
    midi_out_ports: Vec<String>,
    selected_midi_port: Option<String>,
    theme: CustomTheme,
}

impl Gui {
    fn new(
        tx_gui: Arc<Mutex<Option<mpsc::Sender<Message>>>>,
        tx_midi: Sender<MidiCommand>,
        sequencer_left: EuclideanGui,
        sequencer_right: EuclideanGui,
    ) -> Self {
        Self {
            tx_gui,
            tx_midi,
            sequencer_left,
            sequencer_right,
            mixer_ratio: 0.5,
            current_note_index: 0,
            midi_out_ports: vec!["".to_string()],
            selected_midi_port: None,
            theme: CustomTheme::default(),
        }
    }
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::run(poll).map(Message::ReceivedEvent)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ReceivedEvent(event) => match event {
                Event::Connected(sender) => {
                    info!("Sender connected!");
                    if let Ok(mut guard) = self.tx_gui.lock() {
                        *guard = Some(sender.clone());
                    }
                }
                Event::Disconnected => info!("Sender Disconnected"),
            },
            Message::NotePlayed(note) => self.current_note_index = note,
            Message::LeftSequencer(state) => {
                self.sequencer_left
                    .update(EuclideanGuiMessage::UpdateState(state));
            }
            Message::RightSequencer(state) => {
                info!("Right sequencer message in Main GUI update: {:?}", state)
            }
            Message::MixerRatioChanged(ratio) => {
                self.mixer_ratio = ratio;
            }
            Message::RefreshMidiPorts => {
                info!("Sending GetPorts");
                let tx_midi = self.tx_midi.clone();

                return Task::perform(
                    async move {
                        let (tx_oneshot, rx_oneshot) = oneshot::channel();
                        if let Err(e) = tx_midi
                            .send(MidiCommand::GetPorts {
                                responder: tx_oneshot,
                            })
                            .await
                        {
                            warn!("Could not send ports oneshot to GUI: {e}");
                        }

                        match rx_oneshot.await {
                            Ok(ports) => Message::MidiPortsLoaded(Ok(ports)),
                            Err(e) => Message::MidiPortsLoaded(Err(format!(
                                "Oneshot receive error: {e}"
                            ))),
                        }
                    },
                    |msg| msg,
                );
            }
            Message::MidiPortsLoaded(result) => match result {
                Ok(ports) => {
                    self.midi_out_ports = ports;
                    info!(
                        "Successfully received new ports: {:?}",
                        self.midi_out_ports
                    );
                }
                Err(e) => {
                    warn!("Failed to load ports: {}", e);
                }
            },
            Message::MidiPortSelected(port) => {
                let tx_midi = self.tx_midi.clone();
                let port_to_set = port.clone();

                return Task::perform(
                    async move {
                        info!("Sending SetPort");
                        match tx_midi
                            .send(MidiCommand::SetPort {
                                out_port: port_to_set.clone(),
                            })
                            .await
                        {
                            Ok(_) => Message::MidiPortSet(port_to_set),
                            Err(e) => Message::ErrorOccurred(format!(
                                "Could not send SetPort message: {e}"
                            )),
                        }
                    },
                    |msg| msg,
                );
            }
            Message::ErrorOccurred(err) => {
                error!("Received error: {}", err);
            }
            Message::MidiPortSet(port) => {
                self.selected_midi_port = Some(port);
            }
        }

        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
        let sequencer_left_view = Container::new(
            self.sequencer_left.view().map(Message::LeftSequencer),
        )
        .width(Length::FillPortion(1))
        .height(Length::Fill);

        let sequencer_right_view = Container::new(
            self.sequencer_right.view().map(Message::RightSequencer),
        )
        .width(Length::FillPortion(1))
        .height(Length::Fill);

        let sequencer_content =
            row![sequencer_left_view, sequencer_right_view].spacing(20);

        let mixer_content = Container::new(self.view_mixer())
            .width(Length::Fill)
            .height(Length::Fill);

        let midi_content = Container::new(self.view_midi())
            .width(Length::Fill)
            .height(Length::Fill);

        let general_help_text = column![
            text("General")
                .color(self.theme.secondary_text_color)
                .font(self.theme.bold_font)
                .size(self.theme.header_text_size),
            text("Spacebar: resume / pause playback\nTab: change active sequencer\nCtrl+C: exit program")
                .color(self.theme.text_color)
                .size(self.theme.text_size),
            text("Active sequencer")
                .color(self.theme.secondary_text_color)
                .font(self.theme.bold_font).size(self.theme.header_text_size),
            text("W / S: increase / decrease pitch by 1 step\nD / A: increase / decrease octave by 1")
                .color(self.theme.text_color)
                .size(self.theme.text_size),
        ];

        let sequencer_help_text = column![
            text("Euclidean sequencer")
                .color(self.theme.secondary_text_color)
                .font(self.theme.bold_font)
                .size(self.theme.header_text_size),
            text("Up / Down: increase / decrease steps\nRight / Left: increase / decrease pulses")
                .color(self.theme.text_color)
                .size(self.theme.text_size),
            text("Mixer")
                .color(self.theme.secondary_text_color)
                .font(self.theme.bold_font)
                .size(self.theme.header_text_size),
            text("R / F: increase / decrease mixer ratio")
                .color(self.theme.text_color)
                .size(self.theme.text_size),
        ];

        let help_text_content = column![
            text("Controls")
                .color(self.theme.primary_text_color)
                .font(self.theme.header_font)
                .align_y(Start)
                .size(self.theme.header_text_size),
            row![general_help_text, sequencer_help_text]
        ];

        let content = column![
            sequencer_content.height(Length::FillPortion(3)),
            mixer_content.height(Length::FillPortion(1)),
            midi_content.height(Length::FillPortion(1)),
            help_text_content.height(Length::FillPortion(1))
        ]
        .spacing(1)
        .align_x(Center);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn run(
        tx_gui: Arc<Mutex<Option<mpsc::Sender<Message>>>>,
        tx_midi: Sender<MidiCommand>,
        sequencer_left: EuclideanGui,
        sequencer_right: EuclideanGui,
    ) -> iced::Result {
        iced::application("Sequencer", Gui::update, Gui::view)
            .subscription(|gui| gui.subscription())
            .theme(|_| Theme::Dark)
            .antialiasing(true)
            .centered()
            .run_with(|| {
                (
                    Self::new(tx_gui, tx_midi, sequencer_left, sequencer_right),
                    Task::none(),
                )
            })
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Connected(mpsc::Sender<Message>),
    Disconnected,
}

fn poll() -> impl Stream<Item = Event> {
    stream::channel(100, |mut output| async move {
        let (sender, mut receiver) = mpsc::channel(100);

        if let Err(e) = output.send(Event::Connected(sender)).await {
            error!("Error sending Event::Connected: {}", e);
        }

        loop {
            use iced_futures::futures::StreamExt;

            if let Message::ReceivedEvent(event) =
                receiver.select_next_some().await
            {
                output
                    .send(event)
                    .await
                    .expect("Failed to send Message::ReceivedEvent");
            };
        }
    })
}

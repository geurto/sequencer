use crate::{
    midi::gui::{Gui as MidiGui, Message as MidiGuiMessage},
    sequencers::{
        euclidean::gui::{Gui as EuclideanGui, Message as EuclideanGuiMessage},
        mixer::gui::{Gui as MixerGui, Message as MixerGuiMessage},
    },
    SharedState,
};
use iced::{
    color,
    futures::{channel::mpsc, SinkExt, Stream},
    stream,
    widget::{column, container, row, text, vertical_space, Container},
    Alignment::{Center, Start},
    Color, Element, Font, Length, Subscription, Task, Theme,
};
use iced_futures::core::font;
use log::{error, info};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub enum Message {
    ReceivedEvent(Event),
    LeftSequencer(EuclideanGuiMessage),
    RightSequencer(EuclideanGuiMessage),
    Mixer(MixerGuiMessage),
    MidiConnection(MidiGuiMessage),
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
}

impl Default for CustomTheme {
    fn default() -> Self {
        // All colors taken from Catppuccin Mocha
        Self {
            primary_color: color!(0xcba6f7),       // Mauve
            primary_color_muted: color!(0x65537b), // Muted mauve

            secondary_color: color!(0xf5c2e7),       // Pink
            secondary_color_muted: color!(0x7a6173), // Muted pink

            primary_text_color: color!(0x89b4fa),   // Blue
            secondary_text_color: color!(0xb4befe), // Lavender
            text_color: color!(0xcdd6f4),           // Text,

            surface_color: color!(0x1e1e2e), // Base
            overlay_color: color!(0x313244), // Surface0

            accent_color: color!(0xb4befe),       // Lavender
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
        }
    }
}

pub struct Gui {
    tx_gui: Arc<Mutex<Option<mpsc::Sender<Message>>>>,
    cached_state: Option<SharedState>,
    sequencer_left: EuclideanGui,
    sequencer_right: EuclideanGui,
    mixer: MixerGui,
    midi: MidiGui,
    theme: CustomTheme,
}

impl Gui {
    fn new(
        tx_gui: Arc<Mutex<Option<mpsc::Sender<Message>>>>,
        sequencer_left: EuclideanGui,
        sequencer_right: EuclideanGui,
        mixer: MixerGui,
        midi: MidiGui,
    ) -> Self {
        Self {
            tx_gui,
            cached_state: None,
            sequencer_left,
            sequencer_right,
            mixer,
            midi,
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
                Event::StateChanged(state) => {
                    self.cached_state = Some(state.clone());

                    self.sequencer_left
                        .update(EuclideanGuiMessage::FromApp(state.clone()));
                    self.sequencer_right
                        .update(EuclideanGuiMessage::FromApp(state.clone()));
                    self.mixer
                        .update(MixerGuiMessage::FromApp(state.mixer_state));
                }
            },
            Message::LeftSequencer(state) => {
                info!("Left sequencer message in Main GUI update: {:?}", state)
            }
            Message::RightSequencer(state) => {
                info!("Right sequencer message in Main GUI update: {:?}", state)
            }
            Message::Mixer(state) => {
                info!("Mixer message in Main GUI update: {:?}", state)
            }
            Message::MidiConnection(connection) => {
                info!(
                    "MIDI connection message in Main GUI update: {:?}",
                    connection
                );
                self.midi.update(connection);
            }
        }

        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
        let sequencer_left_view =
            Container::new(self.sequencer_left.view().map(Message::LeftSequencer))
                .width(Length::FillPortion(1))
                .height(Length::Fill);

        let sequencer_right_view =
            Container::new(self.sequencer_right.view().map(Message::RightSequencer))
                .width(Length::FillPortion(1))
                .height(Length::Fill);

        let sequencer_content = row![sequencer_left_view, sequencer_right_view].spacing(20);

        let mixer_content = Container::new(self.mixer.view().map(Message::Mixer))
            .width(Length::Fill)
            .height(Length::Fill);

        let midi_content = Container::new(self.midi.view().map(Message::MidiConnection))
            .width(Length::Fill)
            .height(Length::Fill);

        let help_text_content = column![
            text("Controls")
                .color(self.theme.primary_text_color)
                .font(self.theme.header_font)
                .align_y(Start),
            vertical_space().height(10),
            text("General").color(self.theme.secondary_text_color).font(self.theme.bold_font),
            text(
                "Spacebar: resume / pause playback\nTab: change active sequencer\nCtrl+C: exit program"
            )
            .color(self.theme.text_color),
            vertical_space().height(20),
            text("Active sequencer").color(self.theme.secondary_text_color).font(self.theme.bold_font),
            text(
                "W / S: increase / decrease pitch by 1 step\nD / A: increase / decrease octave by 1"
            ).color(self.theme.text_color),
            vertical_space().height(20),
            text("Euclidean sequencer").color(self.theme.secondary_text_color).font(self.theme.bold_font),
            text("Up / Down: increase / decrease steps\nRight / Left: increase / decrease pulses").color(self.theme.text_color),
            vertical_space().height(20),
            text("Mixer").color(self.theme.secondary_text_color).font(self.theme.bold_font),
            text("R / F: increase / decrease mixer ratio").color(self.theme.text_color),
            vertical_space().height(80)
        ];

        let content = column![
            sequencer_content,
            mixer_content,
            midi_content,
            help_text_content
        ]
        .align_x(Center)
        .spacing(20);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn run(
        tx_gui: Arc<Mutex<Option<mpsc::Sender<Message>>>>,
        sequencer_left: EuclideanGui,
        sequencer_right: EuclideanGui,
        mixer: MixerGui,
        midi: MidiGui,
    ) -> iced::Result {
        iced::application("Sequencer", Gui::update, Gui::view)
            .subscription(|gui| gui.subscription())
            .theme(|_| Theme::Dark)
            .antialiasing(true)
            .centered()
            .run_with(|| {
                (
                    Self::new(tx_gui, sequencer_left, sequencer_right, mixer, midi),
                    Task::none(),
                )
            })
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Connected(mpsc::Sender<Message>),
    Disconnected,
    StateChanged(SharedState),
}

fn poll() -> impl Stream<Item = Event> {
    stream::channel(100, |mut output| async move {
        let (sender, mut receiver) = mpsc::channel(100);

        if let Err(e) = output.send(Event::Connected(sender)).await {
            error!("Error sending Event::Connected: {}", e);
        }

        loop {
            use iced_futures::futures::StreamExt;

            match receiver.select_next_some().await {
                Message::ReceivedEvent(event) => output
                    .send(event)
                    .await
                    .expect("Failed to send Message::ReceivedEvent"),
                Message::LeftSequencer(msg) => info!("Received Message::LeftSequencer: {:?}", msg),
                Message::RightSequencer(msg) => {
                    info!("Received Message::RightSequencer: {:?}", msg)
                }
                Message::Mixer(msg) => {
                    info!("Received Message::Mixer: {:?}", msg)
                }
                Message::MidiConnection(msg) => {
                    info!("Received Message::MidiConnection: {:?}", msg)
                }
            };
        }
    })
}

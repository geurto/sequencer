pub mod midi;
pub mod mixer;
pub mod sequencers;
pub mod state;
pub mod theme;

use crate::MidiCommand;
use iced::{
    futures::channel::mpsc,
    widget::Container,
    widget::{column, container, row, text},
    Alignment::{Center, Start},
    Element, Length, Subscription, Task, Theme,
};
use log::{error, info, warn};
use sequencers::euclidean::{
    Gui as EuclideanGui, Message as EuclideanGuiMessage,
};
use state::{poll, Event, GuiMessage};
use std::sync::{Arc, Mutex};
use theme::CustomTheme;
use tokio::sync::{mpsc::Sender, oneshot};

pub struct Gui {
    tx_gui: Arc<Mutex<Option<mpsc::Sender<GuiMessage>>>>,
    tx_midi: Sender<MidiCommand>,
    sequencer_left: EuclideanGui,
    sequencer_right: EuclideanGui,
    mixer_ratio: f32,
    midi_out_ports: Vec<String>,
    selected_midi_port: Option<String>,
    theme: CustomTheme,
}

impl Gui {
    fn new(
        tx_gui: Arc<Mutex<Option<mpsc::Sender<GuiMessage>>>>,
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
            midi_out_ports: vec![],
            selected_midi_port: None,
            theme: CustomTheme::default(),
        }
    }
    pub fn subscription(&self) -> Subscription<GuiMessage> {
        Subscription::run(poll).map(GuiMessage::ReceivedEvent)
    }

    pub fn update(&mut self, message: GuiMessage) -> Task<GuiMessage> {
        match message {
            GuiMessage::ReceivedEvent(event) => match event {
                Event::Connected(sender) => {
                    info!("Sender connected!");
                    if let Ok(mut guard) = self.tx_gui.lock() {
                        *guard = Some(sender.clone());
                    }
                }
                Event::Disconnected => info!("Sender Disconnected"),
                Event::StateChanged(state) => {
                    self.sequencer_left.update(
                        EuclideanGuiMessage::UpdateState(state.clone()),
                    );

                    self.sequencer_right.update(
                        EuclideanGuiMessage::UpdateState(state.clone()),
                    );
                    self.mixer_ratio = state.mixer.ratio;
                }
            },
            GuiMessage::LeftSequencer(state) => {
                self.sequencer_left.update(state);
            }
            GuiMessage::RightSequencer(state) => {
                self.sequencer_right.update(state);
            }
            GuiMessage::MixerRatioChanged(ratio) => {
                self.mixer_ratio = ratio;
            }
            GuiMessage::RefreshMidiPorts => {
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
                            Ok(ports) => GuiMessage::MidiPortsLoaded(Ok(ports)),
                            Err(e) => GuiMessage::MidiPortsLoaded(Err(
                                format!("Oneshot receive error: {e}"),
                            )),
                        }
                    },
                    |msg| msg,
                );
            }
            GuiMessage::MidiPortsLoaded(result) => match result {
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
            GuiMessage::MidiPortSelected(port) => {
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
                            Ok(_) => GuiMessage::MidiPortSet(port_to_set),
                            Err(e) => GuiMessage::ErrorOccurred(format!(
                                "Could not send SetPort message: {e}"
                            )),
                        }
                    },
                    |msg| msg,
                );
            }
            GuiMessage::ErrorOccurred(err) => {
                error!("Received error: {}", err);
            }
            GuiMessage::MidiPortSet(port) => {
                self.selected_midi_port = Some(port);
            }
        }

        Task::none()
    }

    pub fn view(&self) -> Element<GuiMessage> {
        let sequencer_left_view = Container::new(
            self.sequencer_left.view().map(GuiMessage::LeftSequencer),
        )
        .width(Length::FillPortion(1))
        .height(Length::Fill);

        let sequencer_right_view = Container::new(
            self.sequencer_right.view().map(GuiMessage::RightSequencer),
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
            text("Up / Down: increase / decrease steps\nRight / Left: increase / decrease pulses\n] / [: increase / decrease phase")
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
        tx_gui: Arc<Mutex<Option<mpsc::Sender<GuiMessage>>>>,
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

use crate::{
    sequencers::euclidean::gui::{Gui as EuclideanGui, Message as EuclideanGuiMessage},
    state::SharedState,
};
use iced::{
    widget::{container, row, Container},
    Element, Length, Subscription, Theme,
};
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum Message {
    Tick,
}

pub struct Gui {
    sequencer_left: EuclideanGui,
    sequencer_right: EuclideanGui,
}

impl Gui {
    fn new(sequencer_left: EuclideanGui, sequencer_right: EuclideanGui) -> Self {
        Self {
            sequencer_left,
            sequencer_right,
        }
    }
    pub fn subscription(&self) -> Subscription<Message> {
        // @todo subscribe to Tick messages from Sequencer GUIs and Mixer GUI
        Subscription::none()
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Tick => {}
        }
    }

    pub fn view(&self) -> Element<Message> {
        let sequencer_left_view = Container::new(self.sequencer_left.view().map(
            |msg: EuclideanGuiMessage| match msg {
                EuclideanGuiMessage::StateChange(state) => Message::StateChange(SharedState {
                    left_state: state,
                    ..Default::default()
                }),
            },
        ))
        .width(Length::FillPortion(1))
        .height(Length::Fill);

        let sequencer_right_view = Container::new(self.sequencer_right.view().map(
            |msg: EuclideanGuiMessage| match msg {
                EuclideanGuiMessage::StateChange(state) => Message::StateChange(SharedState {
                    right_state: state,
                    ..Default::default()
                }),
            },
        ))
        .width(Length::FillPortion(1))
        .height(Length::Fill);

        let content = row![sequencer_left_view, sequencer_right_view].spacing(20);
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn run(state_rx: mpsc::Receiver<SharedState>) -> iced::Result {
        iced::application("Sequencer", Gui::update, Gui::view)
            .subscription(|gui| gui.subscription())
            .theme(|_| Theme::Dark)
            .antialiasing(true)
            .centered()
            .run()
    }
}

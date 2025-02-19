use crate::{
    sequencers::euclidean::{
        gui::{Gui as EuclideanGui, Message as EuclideanGuiMessage},
        state::EuclideanSequencerState,
    },
    state::SharedState,
};
use iced::{
    advanced::subscription,
    widget::{container, row, Container},
    Element, Length, Subscription, Theme,
};
use rustc_hash::FxHasher;
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum Message {
    StateChange(SharedState),
}

pub struct Gui {
    // config_rx is an Option in order to consume it when creating the Subscription.
    // mpsc::Receiver does not allow cloning, so this is the best way to pass it through.
    config_rx: Option<mpsc::Receiver<SharedState>>,
    sequencer_left: EuclideanGui,
    sequencer_right: EuclideanGui,
}

impl Gui {
    fn new(config_rx: mpsc::Receiver<SharedState>) -> Self {
        Self {
            config_rx: Some(config_rx),
            sequencer_left: EuclideanGui::new(1),
            sequencer_right: EuclideanGui::new(2),
        }
    }
    pub fn subscription(&mut self) -> Subscription<Message> {
        if let Some(rx) = self.config_rx.take() {
            iced::advanced::subscription::from_recipe(StateSubscription::new(rx))
        } else {
            Subscription::none()
        }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::StateChange(state) => {
                self.sequencer_left
                    .update(EuclideanGuiMessage::StateChange(state.left_state));
                self.sequencer_right
                    .update(EuclideanGuiMessage::StateChange(state.right_state));
            }
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
            .run_with(move || {
                let app = Gui::new(state_rx);
                (app, iced::Task::none())
            })
    }
}

pub struct StateSubscription {
    rx: mpsc::Receiver<SharedState>,
}

impl StateSubscription {
    pub fn new(rx: mpsc::Receiver<SharedState>) -> Self {
        Self { rx }
    }
}

impl subscription::Recipe for StateSubscription {
    type Output = Message;

    fn hash(&self, state: &mut FxHasher) {
        use std::hash::Hash;
        std::any::TypeId::of::<Self>().hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: subscription::EventStream,
    ) -> iced::advanced::graphics::futures::BoxStream<Self::Output> {
        let rx = self.rx;

        Box::pin(iced::futures::stream::unfold(rx, |mut rx| async move {
            rx.recv()
                .await
                .map(|state| (Message::StateChange(state), rx))
        }))
    }
}

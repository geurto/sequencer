use crate::{
    sequencers::euclidean::gui::{Gui as EuclideanGui, Message as EuclideanGuiMessage},
    state::SharedState,
};
use iced::{
    advanced::subscription,
    widget::{container, row, Container},
    Element, Length, Subscription, Theme,
};
use rustc_hash::FxHasher;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

#[derive(Debug)]
pub enum Message {
    StateChange(SharedState),
}

pub struct Gui {
    // We wrap the mpsc receiver in an Arc<Mutex<>> so that it can be shared
    config_rx: Arc<Mutex<mpsc::Receiver<SharedState>>>,
    sequencer_left: EuclideanGui,
    sequencer_right: EuclideanGui,
}

impl Gui {
    fn new(config_rx: mpsc::Receiver<SharedState>) -> Self {
        Self {
            config_rx: Arc::new(Mutex::new(config_rx)),
            sequencer_left: EuclideanGui::new(1),
            sequencer_right: EuclideanGui::new(2),
        }
    }
    pub fn subscription(&self) -> Subscription<Message> {
        let rx = self.config_rx.clone();
        iced::advanced::subscription::from_recipe(StateSubscription::new(rx))
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
    rx: Arc<Mutex<mpsc::Receiver<SharedState>>>,
}

impl StateSubscription {
    pub fn new(rx: Arc<Mutex<mpsc::Receiver<SharedState>>>) -> Self {
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
        use iced::futures::{stream, StreamExt};

        let rx = self.rx;

        stream::unfold(rx, move |rx| async move {
            let next_state = {
                let mut guard = rx.lock().await;
                guard.recv().await
            };

            //if let Some(state) = next_state {
            //    Some((Message::StateChange(state), rx))
            //} else {
            //    None
            //}
            next_state.map(|state| (Message::StateChange(state), rx))
        })
        .boxed()
    }
}

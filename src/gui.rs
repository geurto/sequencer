use crate::{
    sequencers::euclidean::gui::{Gui as EuclideanGui, Message as EuclideanGuiMessage},
    SharedState,
};
use iced::{
    futures::{channel::mpsc, SinkExt, Stream},
    stream,
    widget::{container, row, Container},
    Element, Length, Subscription, Task, Theme,
};
use log::{error, info};
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

#[derive(Debug)]
pub enum Message {
    ReceivedEvent(Event),
    LeftSequencer(EuclideanGuiMessage),
    RightSequencer(EuclideanGuiMessage),
}

pub struct Gui {
    tx_gui: Arc<Mutex<Option<mpsc::Sender<Message>>>>,
    shared_state: Arc<RwLock<SharedState>>,
    cached_state: Option<SharedState>,
    sequencer_left: EuclideanGui,
    sequencer_right: EuclideanGui,
}

impl Gui {
    fn new(
        tx_gui: Arc<Mutex<Option<mpsc::Sender<Message>>>>,
        shared_state: Arc<RwLock<SharedState>>,
        sequencer_left: EuclideanGui,
        sequencer_right: EuclideanGui,
    ) -> Self {
        Self {
            tx_gui,
            shared_state,
            cached_state: None,
            sequencer_left,
            sequencer_right,
        }
    }
    pub fn subscription(&self) -> Subscription<Message> {
        info!("Main GUI subscription");
        Subscription::run(poll).map(Message::ReceivedEvent)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        info!("Main GUI update");
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
                    info!("Received state");
                    self.cached_state = Some(state.clone());

                    self.sequencer_left
                        .update(EuclideanGuiMessage::FromApp(state.left_state));
                    self.sequencer_right
                        .update(EuclideanGuiMessage::FromApp(state.right_state));
                }
            },
            Message::LeftSequencer(state) => {
                info!("Left sequencer message in Main GUI update: {:?}", state)
            }
            Message::RightSequencer(state) => {
                info!("Right sequencer message in Main GUI update: {:?}", state)
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

        let content = row![sequencer_left_view, sequencer_right_view].spacing(20);
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn run(
        tx_gui: Arc<Mutex<Option<mpsc::Sender<Message>>>>,
        shared_state: Arc<RwLock<SharedState>>,
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
                    Self::new(tx_gui, shared_state, sequencer_left, sequencer_right),
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

            let input = receiver.select_next_some().await;
            info!("poll received input {:?}", input);
        }
    })
}

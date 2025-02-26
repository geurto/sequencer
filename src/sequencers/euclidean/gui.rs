use iced::{
    advanced::subscription,
    futures::{channel::mpsc, stream::BoxStream, SinkExt, StreamExt},
    stream,
    widget::{
        canvas::{self, Canvas, Frame, Path},
        column, container, text,
    },
    Alignment::Center,
    Color, Element, Length, Point, Renderer, Subscription,
};

use log::{error, info};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

use super::state::EuclideanSequencerState;

const FILL_COLOR: Color = Color::from_rgb(0.46, 0.23, 0.54);
const BACKGROUND_COLOR: Color = Color::from_rgb(0., 0., 0.);
const CIRCLE_RADIUS: f32 = 20.0;
const CIRCLE_SPACING: f32 = 60.0;

#[derive(Debug, Clone)]
pub enum Message {
    FromApp(Event),
}

pub struct Gui {
    state: EuclideanSequencerState,
    index: usize,
    rx_state: broadcast::Receiver<EuclideanSequencerState>,
}

impl Gui {
    pub fn new(index: usize, rx_state: broadcast::Receiver<EuclideanSequencerState>) -> Self {
        Self {
            state: EuclideanSequencerState::new(),
            index,
            rx_state,
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        info!("Creating subscription for EuclideanGui #{}", self.index);

        let rx = self.rx_state.resubscribe();

        // Create a unique ID for this subscription
        let id = format!("euclidean-sequencer-{}", self.index);

        // Use a custom subscription that doesn't require a function pointer
        subscription_with_receiver(id, rx).map(Message::FromApp)
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::FromApp(Event::StateChange(new_state)) => {
                info!(
                    "EuclideanGui #{}: Updating state: pulses={}, steps={}",
                    self.index, new_state.pulses, new_state.steps
                );
                self.state = new_state;
            }
            Message::FromApp(event) => {
                info!(
                    "EuclideanGui #{}: got FromApp message with event {:?}",
                    self.index, event
                );
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let canvas = Canvas::new(self).width(Length::Fill).height(Length::Fill);
        let content = column![canvas, text!("Sequencer {0}", self.index)];
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Center)
            .align_y(Center)
            .into()
    }
}

impl canvas::Program<Message> for Gui {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        let beat_locations = (0..self.state.pulses)
            .map(|i| (i * self.state.steps) / self.state.pulses)
            .collect::<Vec<_>>();

        for row in 0..4 {
            for col in 0..4 {
                let center = Point::new(
                    CIRCLE_SPACING * (col as f32 + 0.5),
                    CIRCLE_SPACING * (row as f32 + 0.5),
                );

                let circle = Path::circle(center, CIRCLE_RADIUS);
                let color = if beat_locations.contains(&(4 * row + col)) {
                    FILL_COLOR
                } else {
                    BACKGROUND_COLOR
                };

                let bg_circle = Path::circle(center, CIRCLE_RADIUS + 2.);
                frame.fill(&bg_circle, FILL_COLOR);
                frame.fill(&circle, color);
            }
        }
        vec![frame.into_geometry()]
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Connected(mpsc::Sender<Message>),
    Disconnected,
    StateChange(EuclideanSequencerState),
}

fn subscription_with_receiver(
    id: String,
    rx: broadcast::Receiver<EuclideanSequencerState>,
) -> Subscription<Event> {
    struct ReceiverSubscription {
        id: String,
        rx: Arc<Mutex<Option<broadcast::Receiver<EuclideanSequencerState>>>>,
    }

    impl<H, I> subscription::Recipe<H, I> for ReceiverSubscription
    where
        H: std::hash::Hasher,
    {
        type Output = Event;

        fn hash(&self, state: &mut H) {
            use std::hash::Hash;
            self.id.hash(state);
        }

        fn stream(
            self: Box<Self>,
            _input: iced::advanced::subscription::EventStream,
        ) -> BoxStream<Self::Output> {
            let rx = self.rx.lock().unwrap().take().unwrap();

            stream::channel(100, move |mut output| {
                let mut rx = rx;

                async move {
                    let (sender, mut receiver) = mpsc::channel(100);

                    // Send the Connected event
                    if let Err(_) = output.send(Event::Connected(sender.clone())).await {
                        return;
                    }

                    // Create a task to handle broadcast messages
                    let output_clone = output.clone();
                    let broadcast_task = iced::futures::executor::spawn(async move {
                        loop {
                            match rx.recv().await {
                                Ok(state) => {
                                    if let Err(_) =
                                        output_clone.send(Event::StateChange(state)).await
                                    {
                                        break;
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to receive message: {:?}", e);
                                }
                            }
                        }
                    });

                    // Handle messages from the application
                    while let Some(msg) = receiver.next().await {
                        match msg {
                            Message::FromApp(event) => {
                                if let Err(_) = output.send(event).await {
                                    break;
                                }
                            }
                        }
                    }

                    // Clean up
                    broadcast_task.cancel();
                    let _ = output.send(Event::Disconnected).await;
                }
            })
            .boxed()
        }
    }

    subscription::from_recipe(ReceiverSubscription {
        id,
        rx: Arc::new(Mutex::new(Some(rx))),
    })
}

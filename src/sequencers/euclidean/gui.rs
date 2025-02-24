use iced::{
    advanced::subscription,
    widget::canvas::{self, Canvas, Frame, Path},
    widget::{column, container, text},
    Alignment::Center,
    Color, Element, Length, Point, Renderer, Subscription,
};

use rustc_hash::FxHasher;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use super::state::EuclideanSequencerState;

const FILL_COLOR: Color = Color::from_rgb(0.46, 0.23, 0.54);
const BACKGROUND_COLOR: Color = Color::from_rgb(0., 0., 0.);
const CIRCLE_RADIUS: f32 = 20.0;
const CIRCLE_SPACING: f32 = 60.0;

#[derive(Debug, Clone)]
pub enum Message {
    StateChange(EuclideanSequencerState),
}

pub struct Gui {
    state: EuclideanSequencerState,
    rx_state: Arc<Mutex<mpsc::Receiver<EuclideanSequencerState>>>,
    index: usize,
}

impl Gui {
    fn new(rx_state: mpsc::Receiver<EuclideanSequencerState>) -> Self {
        Self {
            state: EuclideanSequencerState::new(),
            rx_state: Arc::new(Mutex::new(rx_state)),
            index,
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let rx = self.rx_state.clone();
        iced::advanced::subscription::from_recipe(StateSubscription::new(rx))
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::StateChange(state) => {}
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

pub struct StateSubscription {
    rx: Arc<Mutex<mpsc::Receiver<EuclideanSequencerState>>>,
}

impl StateSubscription {
    pub fn new(rx: Arc<Mutex<mpsc::Receiver<EuclideanSequencerState>>>) -> Self {
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
            next_state.map(|state| (Message::StateChange(state), rx))
        })
        .boxed()
    }
}

use iced::{
    futures::{channel::mpsc, SinkExt, Stream},
    stream,
    widget::{
        canvas::{self, Canvas, Frame, Path},
        column, container, text,
    },
    Alignment::Center,
    Color, Element, Length, Point, Renderer, Subscription,
};

use log::info;

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
}

impl Gui {
    pub fn new(index: usize) -> Self {
        Self {
            state: EuclideanSequencerState::new(),
            index,
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        info!("Creating subscription for EuclideanGui #{}", self.index);
        Subscription::run(poll).map(Message::FromApp)
    }

    pub fn update(&mut self, message: Message) {
        match message {
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

pub fn poll() -> impl Stream<Item = Event> {
    stream::channel(100, |mut output| async move {
        let (sender, mut receiver) = mpsc::channel(100);

        output
            .send(Event::Connected(sender))
            .await
            .expect("Failed to send Connected event");

        loop {
            use iced_futures::futures::StreamExt;

            let msg = receiver.select_next_some().await;

            match msg {
                Message::FromApp(event) => {
                    output.send(event).await.expect("Failed to send event");
                }
            }
        }
    })
}

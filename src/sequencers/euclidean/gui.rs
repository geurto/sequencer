use iced::advanced::subscription;
use iced::widget::canvas::{self, Canvas, Frame, Path};
use iced::widget::{column, container, text};
use iced::Alignment::Center;
use iced::{Color, Element, Length, Point, Renderer, Subscription};
use rustc_hash::FxHasher;
use tokio::sync::mpsc;

use super::config::EuclideanSequencerConfig;

const FILL_COLOR: Color = Color::from_rgb(0.46, 0.23, 0.54);
const BACKGROUND_COLOR: Color = Color::from_rgb(0., 0., 0.);
const CIRCLE_RADIUS: f32 = 20.0;
const CIRCLE_SPACING: f32 = 60.0;

#[derive(Debug, Clone)]
pub enum Message {
    ConfigChange(EuclideanSequencerConfig),
}

pub struct Gui {
    config: EuclideanSequencerConfig,
    index: usize,
}

impl Gui {
    pub fn new(index: usize) -> Self {
        Self {
            config: EuclideanSequencerConfig::new(),
            index,
        }
    }
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::ConfigChange(new_config) => self.config = new_config,
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

        let beat_locations = (0..self.config.pulses)
            .map(|i| (i * self.config.steps) / self.config.pulses)
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

pub struct ConfigSubscription {
    rx: mpsc::Receiver<EuclideanSequencerConfig>,
}

impl ConfigSubscription {
    pub fn new(rx: mpsc::Receiver<EuclideanSequencerConfig>) -> Self {
        Self { rx }
    }
}

impl subscription::Recipe for ConfigSubscription {
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
                .map(|config| (Message::ConfigChange(config), rx))
        }))
    }
}

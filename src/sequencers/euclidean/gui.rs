use iced::widget::canvas::{self, Canvas, Frame, Path};
use iced::widget::{Column, Container};
use iced::{Color, Element, Length, Point, Renderer, Subscription, Theme};

use log::debug;

const BACKGROUND_COLOR: Color = Color::from_rgb(0.46, 0.23, 0.54);
const TEXT_COLOR: Color = Color::from_rgb(0.97, 0.97, 0.95);
const CIRCLE_RADIUS: f32 = 20.0;
const CIRCLE_SPACING: f32 = 60.0;

#[derive(Debug, Clone)]
pub enum EuclideanGuiMessage {
    SetPulses([bool; 16]),
}

#[derive(Default)]
pub struct EuclideanGui {
    pub pulses: [bool; 16],
}

impl EuclideanGui {
    pub fn subscription(&self) -> Subscription<EuclideanGuiMessage> {
        Subscription::none()
    }

    pub fn update(&mut self, message: EuclideanGuiMessage) {
        match message {
            EuclideanGuiMessage::SetPulses(pulses) => self.set_pulses(pulses),
        }
    }

    pub fn view(&self) -> Element<EuclideanGuiMessage> {
        let canvas = Canvas::new(self).width(Length::Fill).height(Length::Fill);

        let content = Column::new()
            .push(canvas)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(20);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn set_pulses(&mut self, pulses: [bool; 16]) {
        self.pulses = pulses;
    }
}

impl canvas::Program<EuclideanGuiMessage> for EuclideanGui {
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

        for row in 0..4 {
            for col in 0..4 {
                let center = Point::new(
                    bounds.x + CIRCLE_SPACING * (col as f32 + 0.5),
                    bounds.y + CIRCLE_SPACING * (row as f32 + 0.5),
                );

                let circle = Path::circle(center, CIRCLE_RADIUS);
                let color = if self.pulses[4 * row + col] {
                    TEXT_COLOR
                } else {
                    BACKGROUND_COLOR
                };

                frame.fill(&circle, color);
            }
        }
        vec![frame.into_geometry()]
    }
}

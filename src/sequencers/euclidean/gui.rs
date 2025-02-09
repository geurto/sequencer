use iced::widget::canvas::{self, Canvas, Frame, Path};
use iced::widget::{column, container, text};
use iced::Alignment::Center;
use iced::{Color, Element, Length, Point, Renderer, Subscription};

const FILL_COLOR: Color = Color::from_rgb(0.46, 0.23, 0.54);
const BACKGROUND_COLOR: Color = Color::from_rgb(0., 0., 0.);
const CIRCLE_RADIUS: f32 = 20.0;
const CIRCLE_SPACING: f32 = 60.0;

#[derive(Debug, Clone)]
pub enum EuclideanGuiMessage {
    SetPulses([bool; 16]),
}

#[derive(Default)]
pub struct EuclideanGui {
    index: usize,
    pub pulses: [bool; 16],
}

impl EuclideanGui {
    pub fn new(index: usize) -> Self {
        Self {
            index,
            pulses: [false; 16],
        }
    }
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
        let content = column![canvas, text!("Sequencer {0}", self.index)];
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Center)
            .align_y(Center)
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
                    CIRCLE_SPACING * (col as f32 + 0.5),
                    CIRCLE_SPACING * (row as f32 + 0.5),
                );

                let circle = Path::circle(center, CIRCLE_RADIUS);
                let color = if self.pulses[4 * row + col] {
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

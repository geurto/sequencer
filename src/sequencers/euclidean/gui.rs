use iced::widget::canvas::{self, Canvas, Frame, Path};
use iced::widget::{Column, Container};
use iced::{Color, Element, Length, Point, Renderer, Subscription, Theme};

const BACKGROUND_COLOR: Color = Color::from_rgb(0.46, 0.23, 0.54); // #753A8A
const TEXT_COLOR: Color = Color::from_rgb(0.97, 0.97, 0.95); // #F7F7F2
const CIRCLE_RADIUS: f32 = 20.0;
const CIRCLE_SPACING: f32 = 60.0;

#[derive(Debug, Clone, Copy)]
pub enum EuclideanMessage {
    ChangeSteps(usize),
    ChangePulses(usize),
    ChangePhase(usize),
    ChangePitch(usize),
    ChangeOctave(i8),
}

#[derive(Default)]
pub struct EuclideanGui {
    pub steps: usize,
    pub pulses: usize,
    pub phase: usize,
    pub pitch: u8,
}

impl EuclideanGui {
    fn subscription(&self) -> Subscription<EuclideanMessage> {
        Subscription::none()
    }

    fn update(&mut self, message: EuclideanMessage) {
        match message {
            EuclideanMessage::ChangeSteps(steps) => {
                self.steps = steps;
            }
            _ => {}
        }
    }

    fn view(&self) -> Element<EuclideanMessage> {
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
}
impl canvas::Program<EuclideanMessage> for EuclideanGui {
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
                let color = if (row + col) % 2 == 0 {
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

pub fn run() -> iced::Result {
    iced::application(
        "Euclidean Sequencer",
        EuclideanGui::update,
        EuclideanGui::view,
    )
    .subscription(EuclideanGui::subscription)
    .theme(|_| Theme::Dark)
    .centered()
    .run()
}

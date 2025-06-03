use iced::{
    widget::{
        canvas::{self, Canvas, Frame, Path},
        column, container, text,
    },
    Alignment::Center,
    Color, Element, Length, Point, Renderer, Subscription,
};

use crate::SharedState;

const FILL_COLOR: Color = Color::from_rgb(0.46, 0.23, 0.54);
const INACTIVE_COLOR: Color = Color::from_rgb(0.23, 0.11, 0.27);
const CURRENT_NOTE_COLOR: Color = Color::from_rgb(0.69, 0.34, 0.81);
const BACKGROUND_COLOR: Color = Color::from_rgb(0., 0., 0.);
const CIRCLE_RADIUS: f32 = 20.0;
const CIRCLE_SPACING: f32 = 60.0;

#[derive(Debug, Clone)]
pub enum Message {
    FromApp(SharedState),
}

pub struct Gui {
    state: SharedState,
    index: usize,
}

impl Gui {
    pub fn new(index: usize) -> Self {
        Self {
            state: SharedState::new(120.0),
            index,
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::FromApp(new_state) => {
                self.state = new_state;
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let canvas = Canvas::new(self).width(Length::Fill).height(Length::Fill);
        let content = column![canvas, text!("Sequencer {0}", self.index)].align_x(Center);
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
        let center = frame.center();
        let start_x = center.x - 1.5 * CIRCLE_SPACING - 2. * CIRCLE_RADIUS;
        let start_y = center.y - 1.5 * CIRCLE_SPACING - 2. * CIRCLE_RADIUS;

        let sequencer_state = if self.index == 0 {
            self.state.left_state
        } else {
            self.state.right_state
        };

        let beat_locations = (0..sequencer_state.pulses)
            .map(|i| (i * sequencer_state.steps) / sequencer_state.pulses)
            .collect::<Vec<_>>();

        for row in 0..4 {
            for col in 0..4 {
                let center = Point::new(
                    start_x + CIRCLE_SPACING * (col as f32 + 0.5),
                    start_y + CIRCLE_SPACING * (row as f32 + 0.5),
                );

                let circle = Path::circle(center, CIRCLE_RADIUS);
                let color = if beat_locations.contains(&(4 * row + col)) {
                    FILL_COLOR
                } else if 4 * row + col >= sequencer_state.steps {
                    INACTIVE_COLOR
                } else {
                    BACKGROUND_COLOR
                };

                let bg_circle = Path::circle(center, CIRCLE_RADIUS + 2.);
                frame.fill(&bg_circle, FILL_COLOR);

                if 4 * row + col == self.state.current_note_index % sequencer_state.steps {
                    frame.fill(&circle, CURRENT_NOTE_COLOR);
                } else {
                    frame.fill(&circle, color);
                }
            }
        }
        vec![frame.into_geometry()]
    }
}

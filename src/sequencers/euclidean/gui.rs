use iced::{
    widget::{
        canvas::{self, Canvas, Frame, Path},
        column, container,
    },
    Alignment::Center,
    Color, Element, Length, Point, Renderer, Subscription,
};

use crate::{gui::CustomTheme, state::SequencerSlot, SharedState};

const CIRCLE_RADIUS: f32 = 20.0;
const CIRCLE_SPACING: f32 = 60.0;

#[derive(Debug, Clone)]
pub enum Message {
    FromApp(SharedState),
}

pub struct Gui {
    state: SharedState,
    slot: SequencerSlot,
    theme: CustomTheme,
}

impl Gui {
    pub fn new(slot: SequencerSlot) -> Self {
        Self {
            state: SharedState::new(120.0),
            slot,
            theme: CustomTheme::default(),
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
        let content = column![canvas].align_x(Center);
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

        let sequencer_state = if self.slot == SequencerSlot::Left {
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
                    self.theme.accent_color
                } else if 4 * row + col >= sequencer_state.steps {
                    self.theme.accent_color_muted
                } else {
                    self.theme.surface_color
                };

                let bg_circle = if self.state.active_sequencer == self.slot {
                    Path::circle(center, CIRCLE_RADIUS + 4.)
                } else {
                    Path::circle(center, CIRCLE_RADIUS + 2.)
                };
                frame.fill(&bg_circle, self.theme.primary_color_muted);

                if 4 * row + col == self.state.current_note_index % sequencer_state.steps {
                    frame.fill(&circle, self.theme.primary_color);
                } else {
                    frame.fill(&circle, color);
                }
            }
        }
        vec![frame.into_geometry()]
    }
}

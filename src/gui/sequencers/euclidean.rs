use iced::{
    alignment::{Horizontal, Vertical},
    border::Radius,
    widget::{
        canvas::{self, Canvas, Frame, Path, Text},
        column, container,
    },
    Alignment::Center,
    Element, Length, Point, Renderer, Size, Subscription,
};

use crate::{
    gui::CustomTheme, playback::state::SequencerSlot, EuclideanSequencerState,
    Sequence, SharedState,
};

#[derive(Debug, Clone)]
pub enum Message {
    UpdateState(SharedState),
}

pub struct Gui {
    state: EuclideanSequencerState,
    active_sequencer: bool,
    current_note_index: usize,
    slot: SequencerSlot,
    theme: CustomTheme,
}

impl Gui {
    pub fn new(slot: SequencerSlot) -> Self {
        let active_sequencer: bool = slot == SequencerSlot::Left;
        Self {
            state: EuclideanSequencerState::default(),
            active_sequencer,
            current_note_index: 0,
            slot,
            theme: CustomTheme::default(),
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::UpdateState(new_state) => {
                self.state = match self.slot {
                    SequencerSlot::Left => new_state.left_sequencer,
                    SequencerSlot::Right => new_state.right_sequencer,
                };
                self.active_sequencer = new_state.active_sequencer == self.slot;
                self.current_note_index = new_state.current_note_index;
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
        const CIRCLE_RADIUS: f32 = 20.0;
        const CIRCLE_BORDER_RADIUS: f32 = CIRCLE_RADIUS + 2.0;
        const ACTIVE_CIRCLE_BORDER_RADIUS: f32 = CIRCLE_RADIUS + 4.0;
        const CIRCLE_SPACING: f32 = 60.0;

        let mut frame = Frame::new(renderer, bounds.size());
        let center = frame.center();
        let start_x = center.x - 1.5 * CIRCLE_SPACING - 2. * CIRCLE_RADIUS;
        let start_y = center.y - 1.5 * CIRCLE_SPACING - 2. * CIRCLE_RADIUS;

        let beat_locations = (0..self.state.pulses)
            .map(|i| {
                (self.state.phase + (i * self.state.steps) / self.state.pulses)
                    % self.state.steps
            })
            .collect::<Vec<_>>();

        for row in 0..4 {
            for col in 0..4 {
                let circle_center = Point::new(
                    start_x + CIRCLE_SPACING * (col as f32 + 0.5),
                    start_y + CIRCLE_SPACING * (row as f32 + 0.5),
                );

                let circle = Path::circle(circle_center, CIRCLE_RADIUS);

                // circle outline
                let bg_circle = if self.active_sequencer {
                    Path::circle(circle_center, ACTIVE_CIRCLE_BORDER_RADIUS)
                } else {
                    Path::circle(circle_center, CIRCLE_BORDER_RADIUS)
                };
                frame.fill(&bg_circle, self.theme.primary_color_muted);

                // pulses and current playing note
                let color = if beat_locations.contains(&(4 * row + col)) {
                    self.theme.accent_color
                } else if 4 * row + col >= self.state.steps {
                    self.theme.accent_color_muted
                } else {
                    self.theme.surface_color
                };
                if 4 * row + col == self.current_note_index % self.state.steps {
                    frame.fill(&circle, self.theme.primary_color);
                } else {
                    frame.fill(&circle, color);
                }
            }
        }

        // show note info - rounded rectangle
        const BOX_PADDING_FROM_CIRCLES: f32 = 15.0;
        const BOX_HEIGHT: f32 = 40.0;
        const BOX_CORNER_RADIUS: f32 = 8.0;

        let grid_width = 4. * CIRCLE_SPACING;

        let box_top_left = Point::new(
            start_x,
            start_y + grid_width + BOX_PADDING_FROM_CIRCLES,
        );
        let box_size = Size::new(grid_width, BOX_HEIGHT);

        let rounded_rect_path = Path::rounded_rectangle(
            box_top_left,
            box_size,
            Radius::new(BOX_CORNER_RADIUS),
        );
        frame.fill(&rounded_rect_path, self.theme.primary_color_muted);

        let box_center = Point::new(
            box_top_left.x + box_size.width / 2.0,
            box_top_left.y + box_size.height / 2.0,
        );

        // show note info - text
        let note_info = Sequence::midi_to_note_name(self.state.pitch);
        let text = Text {
            content: note_info,
            position: box_center,
            color: self.theme.primary_text_color,
            size: iced::Pixels(20.0),
            horizontal_alignment: Horizontal::Center,
            vertical_alignment: Vertical::Center,
            ..Text::default()
        };
        frame.fill_text(text);

        vec![frame.into_geometry()]
    }
}

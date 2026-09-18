use iced::{
    Alignment::Center,
    Element, Length, Point, Renderer, Size, Subscription,
    alignment::{Horizontal, Vertical},
    border::Radius,
    widget::{
        canvas::{self, Canvas, Frame, Path, Text},
        column, container,
    },
};
use seq_ui::{Slot, SlotSnapshot, UiSnapshot};

use crate::gui::CustomTheme;

#[derive(Clone, Copy, Debug)]
pub enum Message {
    UpdateState(UiSnapshot),
}

#[derive(Debug)]
pub struct Gui {
    snapshot: SlotSnapshot,
    is_active: bool,
    step_index: usize,
    slot: Slot,
    theme: CustomTheme,
}

impl Gui {
    #[must_use]
    pub fn new(slot: Slot) -> Self {
        Self {
            snapshot: SlotSnapshot::default(),
            is_active: slot == Slot::Left,
            step_index: 0,
            slot,
            theme: CustomTheme::default(),
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::UpdateState(state) => {
                self.snapshot = state.slots[self.slot.index()];
                self.is_active = state.active == self.slot;
                self.step_index = usize::from(state.step_index);
            }
        }
    }

    #[must_use]
    pub fn view(&self) -> Element<'_, Message> {
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
        const BOX_PADDING_FROM_CIRCLES: f32 = 15.0;
        const BOX_HEIGHT: f32 = 40.0;
        const BOX_CORNER_RADIUS: f32 = 8.0;

        let mut frame = Frame::new(renderer, bounds.size());
        let center = frame.center();
        let start_x = center.x - 1.5 * CIRCLE_SPACING - 2. * CIRCLE_RADIUS;
        let start_y = center.y - 1.5 * CIRCLE_SPACING - 2. * CIRCLE_RADIUS;

        let steps = usize::from(self.snapshot.steps).max(1);

        for row in 0..4u16 {
            for col in 0..4u16 {
                let index = usize::from(4 * row + col);
                let circle_center = Point::new(
                    start_x + CIRCLE_SPACING * (f32::from(col) + 0.5),
                    start_y + CIRCLE_SPACING * (f32::from(row) + 0.5),
                );

                let circle = Path::circle(circle_center, CIRCLE_RADIUS);

                // circle outline
                let bg_circle = if self.is_active {
                    Path::circle(circle_center, ACTIVE_CIRCLE_BORDER_RADIUS)
                } else {
                    Path::circle(circle_center, CIRCLE_BORDER_RADIUS)
                };
                frame.fill(&bg_circle, self.theme.primary_color_muted);

                // pulses and current playing note
                let color = if self.snapshot.hit(index) {
                    self.theme.accent_color
                } else if index >= steps {
                    self.theme.accent_color_muted
                } else {
                    self.theme.surface_color
                };
                if index == self.step_index % steps {
                    frame.fill(&circle, self.theme.primary_color);
                } else {
                    frame.fill(&circle, color);
                }
            }
        }

        // show note info - rounded rectangle
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
        let note_info = seq_core::note_name(self.snapshot.pitch).to_string();
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

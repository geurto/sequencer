use crate::sequencers::euclidean::gui::{EuclideanGui, EuclideanGuiMessage};
use iced::widget::canvas::{self, Canvas, Frame, Path};
use iced::widget::{column, row, Column, Container};
use iced::{Color, Element, Length, Point, Renderer, Subscription, Theme};

#[derive(Debug, Clone)]
enum GuiMessage {
    EuclideanGuiMessage(EuclideanGuiMessage),
}

struct Gui {
    sequencer_left_gui: EuclideanGui,
    sequencer_right_gui: EuclideanGui,
}

impl Gui {
    pub fn subscription(&self) -> Subscription<GuiMessage> {
        Subscription::none()
    }

    pub fn update(&mut self, message: GuiMessage) {}

    pub fn view(&self) -> Element<GuiMessage> {
        let sequencer_left_view = self
            .sequencer_left_gui
            .view()
            .map(GuiMessage::EuclideanGuiMessage);
        let sequencer_right_view = self
            .sequencer_right_gui
            .view()
            .map(GuiMessage::EuclideanGuiMessage);
        row![sequencer_left_view, sequencer_right_view].into()
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
}

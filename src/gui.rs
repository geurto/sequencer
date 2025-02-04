use crate::sequencers::euclidean::gui::{EuclideanGui, EuclideanGuiMessage};
use iced::widget::row;
use iced::{Element, Subscription, Theme};

#[derive(Debug, Clone)]
enum GuiMessage {
    EuclideanGuiMessage(EuclideanGuiMessage),
}

pub struct Gui {
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

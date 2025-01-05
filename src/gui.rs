use crate::sequencers::euclidean::gui::{EuclideanGui, EuclideanGuiMessage};
use iced::Theme;

#[derive(Debug, Clone)]
enum GuiMessage {
    EuclideanGuiMessage(EuclideanGuiMessage),
}

pub fn run_gui() -> iced::Result {
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

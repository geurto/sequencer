use crate::sequencers::euclidean::gui::{EuclideanGui, EuclideanGuiMessage};
use iced::{widget::row, Element, Subscription, Theme};

#[derive(Debug, Clone)]
pub enum GuiMessage {
    LeftGuiMessage(EuclideanGuiMessage),
    RightGuiMessage(EuclideanGuiMessage),
}

#[derive(Default)]
pub struct Gui {
    sequencer_left_gui: EuclideanGui,
    sequencer_right_gui: EuclideanGui,
}

impl Gui {
    pub fn subscription(&self) -> Subscription<GuiMessage> {
        Subscription::none()
    }

    pub fn update(&mut self, message: GuiMessage) {
        match message {
            GuiMessage::LeftGuiMessage(msg) => self.sequencer_left_gui.update(msg),
            GuiMessage::RightGuiMessage(msg) => self.sequencer_right_gui.update(msg),
        }
    }

    pub fn view(&self) -> Element<GuiMessage> {
        let sequencer_left_view = self
            .sequencer_left_gui
            .view()
            .map(GuiMessage::LeftGuiMessage);
        let sequencer_right_view = self
            .sequencer_right_gui
            .view()
            .map(GuiMessage::RightGuiMessage);
        row![sequencer_left_view, sequencer_right_view].into()
    }

    pub fn run() -> iced::Result {
        iced::application("Euclidean Sequencer", Gui::update, Gui::view)
            .subscription(Gui::subscription)
            .theme(|_| Theme::Dark)
            .antialiasing(true)
            .centered()
            .run()
    }
}

use crate::sequencers::euclidean::gui::{EuclideanGui, EuclideanGuiMessage};
use iced::{
    widget::{container, row, Container},
    Element, Length, Subscription, Theme,
};

#[derive(Debug, Clone)]
pub enum GuiMessage {
    LeftGuiMessage(EuclideanGuiMessage),
    RightGuiMessage(EuclideanGuiMessage),
}

pub struct Gui {
    sequencer_left: EuclideanGui,
    sequencer_right: EuclideanGui,
}

impl Gui {
    fn new() -> Self {
        Self {
            sequencer_left: EuclideanGui::new(1),
            sequencer_right: EuclideanGui::new(2),
        }
    }
    pub fn subscription(&self) -> Subscription<GuiMessage> {
        Subscription::none()
    }

    pub fn update(&mut self, message: GuiMessage) {
        match message {
            GuiMessage::LeftGuiMessage(msg) => self.sequencer_left.update(msg),
            GuiMessage::RightGuiMessage(msg) => self.sequencer_right.update(msg),
        }
    }

    pub fn view(&self) -> Element<GuiMessage> {
        let sequencer_left_view =
            Container::new(self.sequencer_left.view().map(GuiMessage::LeftGuiMessage))
                .width(Length::FillPortion(1))
                .height(Length::Fill);

        let sequencer_right_view =
            Container::new(self.sequencer_right.view().map(GuiMessage::RightGuiMessage))
                .width(Length::FillPortion(1))
                .height(Length::Fill);

        let content = row![sequencer_left_view, sequencer_right_view].spacing(20);
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
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

impl Default for Gui {
    fn default() -> Self {
        Self::new()
    }
}

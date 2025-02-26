use crate::sequencers::euclidean::gui::{Gui as EuclideanGui, Message as EuclideanGuiMessage};
use iced::{
    widget::{container, row, Container},
    Element, Length, Subscription, Task, Theme,
};
use log::info;

#[derive(Debug)]
pub enum Message {
    Tick,
}

pub struct Gui {
    sequencer_left: EuclideanGui,
    sequencer_right: EuclideanGui,
}

impl Gui {
    fn new(sequencer_left: EuclideanGui, sequencer_right: EuclideanGui) -> Self {
        Self {
            sequencer_left,
            sequencer_right,
        }
    }
    pub fn subscription(&self) -> Subscription<Message> {
        info!("Main GUI subscription");
        let left_sub = self
            .sequencer_left
            .subscription()
            .map(|_child_msg: EuclideanGuiMessage| Message::Tick);

        let right_sub = self
            .sequencer_right
            .subscription()
            .map(|_child_msg: EuclideanGuiMessage| Message::Tick);

        Subscription::batch(vec![left_sub, right_sub])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        info!("Main GUI update");
        match message {
            Message::Tick => {
                //self.sequencer_left.update(EuclideanGuiMessage::Tick);
                //self.sequencer_right.update(EuclideanGuiMessage::Tick);
            }
        }

        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
        let sequencer_left_view = Container::new(self.sequencer_left.view().map(
            |child_msg: EuclideanGuiMessage| match child_msg {
                EuclideanGuiMessage::FromApp(_) => {
                    info!("Left sequencer StateChange view message");
                    Message::Tick
                }
            },
        ))
        .width(Length::FillPortion(1))
        .height(Length::Fill);

        let sequencer_right_view = Container::new(self.sequencer_right.view().map(
            |child_msg: EuclideanGuiMessage| match child_msg {
                EuclideanGuiMessage::FromApp(_) => {
                    info!("Right sequencer StateChange view message");
                    Message::Tick
                }
            },
        ))
        .width(Length::FillPortion(1))
        .height(Length::Fill);

        let content = row![sequencer_left_view, sequencer_right_view].spacing(20);
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn run(gui_left: EuclideanGui, gui_right: EuclideanGui) -> iced::Result {
        iced::application("Sequencer", Gui::update, Gui::view)
            .subscription(|gui| gui.subscription())
            .theme(|_| Theme::Dark)
            .antialiasing(true)
            .centered()
            .run_with(|| {
                let left = gui_left;
                let right = gui_right;

                (Self::new(left, right), Task::none())
            })
    }
}

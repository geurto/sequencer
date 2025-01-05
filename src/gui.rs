use crate::sequencers::euclidean::gui::EuclideanMessage;

#[derive(Debug, Clone)]
enum GuiMessage {
    EuclideanMessage(EuclideanMessage),
}


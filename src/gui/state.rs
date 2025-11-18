use crate::{
    gui::sequencers::euclidean::Message as EuclideanGuiMessage, SharedState,
};
use iced::{
    futures::{channel::mpsc, SinkExt, Stream},
    stream,
};
use log::error;

#[derive(Debug, Clone)]
pub enum GuiMessage {
    ReceivedEvent(Event),
    LeftSequencer(EuclideanGuiMessage),
    RightSequencer(EuclideanGuiMessage),
    MixerRatioChanged(f32),
    RefreshMidiPorts,
    MidiPortsLoaded(Result<Vec<String>, String>),
    MidiPortSelected(String),
    MidiPortSet(String),
    ErrorOccurred(String),
}

#[derive(Debug, Clone)]
pub enum Event {
    Connected(mpsc::Sender<GuiMessage>),
    Disconnected,
    StateChanged(SharedState),
}

pub fn poll() -> impl Stream<Item = Event> {
    stream::channel(100, |mut output| async move {
        let (sender, mut receiver) = mpsc::channel(100);

        if let Err(e) = output.send(Event::Connected(sender)).await {
            error!("Error sending Event::Connected: {}", e);
        }

        loop {
            use iced_futures::futures::StreamExt;

            if let GuiMessage::ReceivedEvent(event) =
                receiver.select_next_some().await
            {
                output
                    .send(event)
                    .await
                    .expect("Failed to send Message::ReceivedEvent");
            };
        }
    })
}

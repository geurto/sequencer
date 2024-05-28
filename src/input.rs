use tokio::sync::mpsc;
use crate::common::Input;

pub async fn handle_user_input(tx: mpsc::Sender<Input>) {
    loop {
        if button_pressed() {}

        if encoder_turned() {}
    }
}

fn button_pressed() -> bool {
    // GPIO logic here with rppal
    false
}

fn encoder_turned() -> bool {
    // GPIO logic here with rppal
    false
}
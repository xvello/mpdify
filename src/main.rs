use mpdify::mpd::listener::Listener;
use mpdify::mpris::handler::MprisHandler;

#[tokio::main]
pub async fn main() -> () {
    pretty_env_logger::init();

    let (mut mpris_handler, mpris_tx) = MprisHandler::new().await;
    tokio::spawn(async move {
        mpris_handler.run().await;
    });

    let mut listener = Listener::new("0.0.0.0:6600".to_string(), vec![mpris_tx]).await;
    listener.run().await;
}

use mpdify::mpd::listener::Listener;

#[tokio::main]
pub async fn main() -> () {
    pretty_env_logger::init();

    let mut listener = Listener::new("0.0.0.0:6600".to_string(), vec![]).await;
    listener.run().await;
}

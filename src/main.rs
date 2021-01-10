use log::debug;
use mpdify::handlers::aspotify::SpotifyHandler;
use mpdify::listeners::http::listener::HttpListener;
use mpdify::listeners::mpd::MpdListener;
use mpdify::util::{IdleBus, Settings};
use tokio_compat_02::FutureExt;

#[tokio::main]
pub async fn main() -> () {
    pretty_env_logger::init();
    let settings = Settings::new().expect("Cannot read settings");
    debug!["Current settings: {:?}", settings];

    let idle_bus = IdleBus::new();
    let (mut spotify, spotify_tx) = SpotifyHandler::new(&settings, idle_bus.clone()).await;
    let handler_tx = vec![spotify_tx.clone()];

    let mut mpd = MpdListener::new(&settings, handler_tx, idle_bus.clone()).await;
    let mut http = HttpListener::new(&settings, spotify_tx);

    let tasks = vec![
        tokio::spawn(async move { spotify.run().await }),
        tokio::spawn(async move { mpd.run().await }),
        tokio::spawn(async move { http.run().compat().await }), // hyper crate
    ];
    futures::future::join_all(tasks).await;
}

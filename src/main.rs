use log::debug;
use mpdify::handlers::artwork::ArtworkHandler;
use mpdify::handlers::aspotify::SpotifyHandler;
use mpdify::handlers::client::{build_aspotify_client, HandlerClient};
use mpdify::listeners::http::listener::HttpListener;
use mpdify::listeners::mpd::MpdListener;
use mpdify::util::{IdleBus, Settings};

#[tokio::main]
pub async fn main() -> () {
    pretty_env_logger::init();
    let settings = Settings::new().expect("Cannot read settings");
    debug!["Current settings: {:?}", settings];

    let client = build_aspotify_client().unwrap();
    let mut handlers = HandlerClient::default();
    let idle_bus = IdleBus::new();

    let (mut spotify, spotify_tx) =
        SpotifyHandler::new(&settings, client.clone(), idle_bus.clone()).await;
    handlers.add(spotify_tx);

    let (mut artwork, artwork_tx) = ArtworkHandler::new(&settings, client.clone())
        .await
        .unwrap();
    handlers.add(artwork_tx);

    let mut mpd = MpdListener::new(&settings, handlers.clone(), idle_bus.clone()).await;
    let mut http = HttpListener::new(&settings, handlers);

    let tasks = vec![
        tokio::spawn(async move { spotify.run().await }),
        tokio::spawn(async move { artwork.run().await }),
        tokio::spawn(async move { mpd.run().await }),
        tokio::spawn(async move { http.run().await }),
    ];
    futures::future::join_all(tasks).await;
}

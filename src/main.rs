use mpdify::handlers::aspotify::SpotifyHandler;
use mpdify::handlers::mpris::{MprisHandler, OFFICIAL_SPOTIFY_DEST};
use mpdify::listeners::mpd::MpdListener;

#[tokio::main]
pub async fn main() -> () {
    pretty_env_logger::init();

    let mpris_target = OFFICIAL_SPOTIFY_DEST.to_string();
    let (mut mpris, mpris_tx) = MprisHandler::new(mpris_target).await;
    let (mut spotify, spotify_tx) = SpotifyHandler::new().await;
    let handler_tx = vec![mpris_tx, spotify_tx];

    let mut mpd = MpdListener::new("0.0.0.0:6600".to_string(), handler_tx).await;

    let tasks = vec![
        tokio::spawn(async move { mpris.run().await }),
        tokio::spawn(async move { spotify.run().await }),
        tokio::spawn(async move { mpd.run().await }),
    ];
    futures::future::join_all(tasks).await;
}

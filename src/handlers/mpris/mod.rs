static MEDIAPLAYER2_PATH: &str = "/org/mpris/MediaPlayer2";
pub static OFFICIAL_SPOTIFY_DEST: &str = "org.mpris.MediaPlayer2.spotify";

mod client;
mod handler;
mod watcher;

pub use handler::MprisHandler;

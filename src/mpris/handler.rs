use crate::mpd::commands::Command;
use crate::mpd::handlers::HandlerError::Unsupported;
use crate::mpd::handlers::{HandlerError, HandlerInput, HandlerOutput, HandlerResult};
use crate::mpris::client::Player;
use dbus::nonblock;
use dbus::nonblock::stdintf::org_freedesktop_dbus::Peer;
use dbus::nonblock::{Proxy, SyncConnection};
use dbus_tokio::connection;
use log::{debug, warn};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};

static MEDIAPLAYER2_PATH: &str = "/org/mpris/MediaPlayer2";
static OFFICIAL_SPOTIFY_DEST: &str = "org.mpris.MediaPlayer2.spotify";

pub struct MprisHandler {
    proxy: Proxy<'static, Arc<SyncConnection>>,
    rx: Receiver<HandlerInput>,
}

impl MprisHandler {
    pub async fn new() -> (Self, Sender<HandlerInput>) {
        // Connect to the D-Bus session bus (this is blocking, unfortunately).
        let (resource, conn) = connection::new_session_sync().unwrap();
        tokio::spawn(async move {
            let err = resource.await;
            panic!("Lost connection to D-Bus: {}", err);
        });
        let proxy = Proxy::new(
            OFFICIAL_SPOTIFY_DEST,
            MEDIAPLAYER2_PATH,
            Duration::from_secs(1),
            conn,
        );
        let (tx, rx) = mpsc::channel(16);
        (MprisHandler { proxy, rx }, tx)
    }

    pub async fn run(&mut self) {
        debug!["Mpris handler entered loop"];

        while let Some(input) = self.rx.recv().await {
            if let Err(err) = input.resp.send(self.execute(input.command).await) {
                warn!["Cannot send response: {:?}", err];
            }
        }
        debug!["Mpris handler exited loop"];
    }

    async fn execute(&mut self, command: Command) -> HandlerResult {
        match command {
            Command::Ping => self.proxy.ping(),
            Command::Pause(true) => self.proxy.pause(),
            Command::Pause(false) => self.proxy.play(),
            Command::Next => self.proxy.next(),
            Command::Previous => self.proxy.previous(),
            _ => return Err(Unsupported),
        }
        .await
        .map_err(|err| HandlerError::FromString(err.message().unwrap().to_string()))
        .map(|_| HandlerOutput::Ok)
    }
}

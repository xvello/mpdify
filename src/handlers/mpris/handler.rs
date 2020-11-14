use crate::handlers::mpris::watcher::MprisWatcher;
use crate::handlers::mpris::MEDIAPLAYER2_PATH;
use crate::mpd_protocol::*;
use dbus::nonblock::stdintf::org_freedesktop_dbus::Peer;
use dbus::nonblock::{Proxy, SyncConnection};
use dbus_tokio::connection;
use log::{debug, warn};
use std::borrow::Borrow;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{broadcast, mpsc};

pub struct MprisHandler {
    target_name: String,
    proxy: Proxy<'static, Arc<SyncConnection>>,
    command_rx: Receiver<HandlerInput>,
    idle_tx: broadcast::Sender<()>,
}

impl MprisHandler {
    pub async fn new(target_name: String) -> (Self, Sender<HandlerInput>) {
        // Connect to the D-Bus session bus
        let (resource, conn) = connection::new_session_sync().unwrap();
        tokio::spawn(async move {
            let err = resource.await;
            panic!("Lost connection to D-Bus: {}", err);
        });
        let proxy = Proxy::new(
            target_name.clone(),
            MEDIAPLAYER2_PATH,
            Duration::from_secs(1),
            conn,
        );
        let (idle_tx, _) = broadcast::channel(16);
        let (command_tx, command_rx) = mpsc::channel(16);
        (
            MprisHandler {
                target_name,
                proxy,
                command_rx,
                idle_tx,
            },
            command_tx,
        )
    }

    pub fn subscribe_idle(&self) -> broadcast::Receiver<()> {
        self.idle_tx.subscribe()
    }

    pub async fn run(&mut self) {
        debug!["Mpris handler entered loop"];

        // Listen to dbus signals for idle management
        let idle_watch = MprisWatcher::new(
            self.proxy.connection.clone(),
            self.idle_tx.clone(),
            self.target_name.borrow(),
        )
        .await
        .unwrap();

        // Loop in incoming commands
        while let Some(input) = self.command_rx.recv().await {
            if let Err(err) = input.resp.send(self.execute(input.command).await) {
                warn!["Cannot send response: {:?}", err];
            }
        }

        idle_watch.close().await.unwrap();
        debug!["Mpris handler exited loop"];
    }

    async fn execute(&mut self, command: Command) -> HandlerResult {
        // Idle: wait until a PropertiesChanged signal is sent on dbus
        if let Command::Idle = command {
            return self
                .idle_tx
                .subscribe()
                .recv()
                .await
                .map(|_| HandlerOutput::Ok)
                .map_err(|err| HandlerError::FromString(err.to_string()));
        }

        // Other commands map to dbus calls
        match command {
            Command::Ping => self.proxy.ping(),

            /* Use spotify API instead to avoid racing against it
            Command::Pause(Some(true)) => self.proxy.pause(),
            Command::Pause(Some(false)) => self.proxy.play(),
            Command::Pause(None) => self.proxy.play_pause(),
            Command::Play(None) => self.proxy.play(),
            Command::Next => self.proxy.next(),
            Command::Previous => self.proxy.previous(),
            */
            _ => return Err(HandlerError::Unsupported),
        }
        .and_then(|_| Ok(HandlerOutput::Ok))
        .await
        .map_err(|err| HandlerError::FromString(err.message().unwrap().to_string()))
    }
}

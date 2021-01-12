use crate::handlers::client::HandlerClient;
use crate::listeners::mpd::connection::Connection;
use crate::mpd_protocol::*;
use crate::util::{IdleBus, Settings};
use log::{debug, warn};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

pub struct MpdListener {
    tcp_listener: TcpListener,
    handler: HandlerClient,
    idle_bus: Arc<IdleBus>,
}

/// Listens to incoming connections and spawns one Connection task by client
impl MpdListener {
    pub async fn new(
        settings: &Settings,
        mut handler: HandlerClient,
        idle_bus: Arc<IdleBus>,
    ) -> Self {
        // Run basic fallback handler
        let (tx, rx) = mpsc::channel(8);
        handler.add(tx);
        tokio::spawn(async move {
            BasicCommandHandler::run(rx).await;
        });

        MpdListener {
            tcp_listener: TcpListener::bind(settings.mpd_address()).await.unwrap(),
            handler,
            idle_bus,
        }
    }

    pub fn get_address(&self) -> std::io::Result<String> {
        Ok(self.tcp_listener.local_addr()?.to_string())
    }

    pub async fn run(&mut self) {
        debug!["Listening on {}", self.get_address().unwrap_or_default()];
        loop {
            let (socket, _) = self.tcp_listener.accept().await.unwrap();
            let copied_handlers = self.handler.to_owned();
            let idle_messages = self.idle_bus.subscribe();
            tokio::spawn(async move {
                Connection::new(socket, copied_handlers, idle_messages)
                    .run()
                    .await;
            });
        }
    }
}

/// Handles the ping and close commands
pub struct BasicCommandHandler {}

impl BasicCommandHandler {
    async fn run(mut commands: mpsc::Receiver<HandlerInput>) {
        debug!["BasicCommandHandler entered loop"];
        while let Some(input) = commands.recv().await {
            let resp = match input.command {
                Command::Ping => Ok(HandlerOutput::Ok),
                Command::Close => Ok(HandlerOutput::Close),
                Command::Commands => Ok(HandlerOutput::Lines(
                    Command::known_commands()
                        .iter()
                        .map(|s| format!["command: {}", s])
                        .collect(),
                )),
                _ => Err(HandlerError::Unsupported),
            };
            match input.resp.send(resp) {
                Ok(_) => {}
                Err(err) => {
                    warn!["Cannot send response: {:?}", err];
                }
            }
        }
        debug!["BasicCommandHandler exited loop"];
    }
}

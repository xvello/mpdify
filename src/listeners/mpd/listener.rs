use crate::listeners::mpd::connection::Connection;
use crate::listeners::mpd::types::Handlers;
use crate::mpd_protocol::*;
use log::{debug, warn};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

pub struct MpdListener {
    tcp_listener: TcpListener,
    command_handlers: Handlers,
}

/// Listens to incoming connections and spawns one Connection task by client
impl MpdListener {
    pub async fn new(address: String, mut handlers: Handlers) -> Self {
        // Run basic fallback handler
        let (tx, rx) = mpsc::channel(8);
        handlers.push(tx);
        tokio::spawn(async move {
            debug!["starting"];
            BasicCommandHandler::run(rx).await;
        });

        MpdListener {
            tcp_listener: TcpListener::bind(address).await.unwrap(),
            command_handlers: handlers,
        }
    }

    pub fn get_address(&self) -> std::io::Result<String> {
        Ok(self.tcp_listener.local_addr()?.to_string())
    }

    pub async fn run(&mut self) {
        loop {
            let (socket, _) = self.tcp_listener.accept().await.unwrap();
            let copied_handlers = self.command_handlers.to_owned();
            tokio::spawn(async move {
                Connection::new(socket, copied_handlers).run().await;
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

use crate::listeners::mpd::types::{Handlers, ListenerError};
use crate::mpd_protocol::*;
use log::{debug, info, warn};
use std::str::FromStr;
use tokio::io::{BufReader, Lines};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio::sync::oneshot;

pub static MPD_HELLO_STRING: &[u8] = b"OK MPD 0.21.25\n";

/// Handles one individual connections and its command flow,
/// run in its own tokio task spawned by MpdListener
pub struct Connection {
    command_handlers: Handlers,
    read_lines: Lines<BufReader<OwnedReadHalf>>,
    write: OwnedWriteHalf,
}

impl Connection {
    pub fn new(socket: TcpStream, handlers: Handlers) -> Self {
        let (read, write) = socket.into_split();
        let read_lines = BufReader::new(read).lines();

        Connection {
            command_handlers: handlers,
            read_lines,
            write,
        }
    }

    pub async fn run(&mut self) {
        debug!("New connection, saying hello");
        if let Err(err) = self.write.write(MPD_HELLO_STRING).await {
            warn!("Unrecoverable error, closing connection: {}", err);
            return;
        }

        loop {
            match self.one().await {
                Ok(closed) => {
                    if closed {
                        break;
                    }
                }
                Err(err) => {
                    warn!("Unrecoverable error, closing connection: {}", err);
                    break;
                }
            }
        }
    }

    async fn one(&mut self) -> Result<bool, ListenerError> {
        let line = self.read_lines.next_line().await?;
        if line.is_none() {
            debug!("Client closed the connection");
            return Ok(true);
        }

        let command_string = line.as_deref().unwrap();

        // FIXME: stop skipping command lists
        if command_string.starts_with("command_list_") {
            debug!["Ignoring command {}", command_string];
            return Ok(false);
        }

        let result = match Command::from_str(command_string) {
            Err(err) => Err(ListenerError::InputError(err)),
            Ok(command) => match self.exec_command(command).await {
                Err(err) => Err(ListenerError::HandlerError(err)),
                Ok(result) => Ok(result),
            },
        };

        match result {
            Ok(HandlerOutput::Close) => {
                debug!("Closing connection due to client command");
                Ok(true)
            }
            Ok(HandlerOutput::Ok) => {
                self.write.write(b"OK\n").await?;
                Ok(false)
            }
            Ok(HandlerOutput::Data(data)) => {
                for (key, value) in data {
                    self.write
                        .write(format!["{}: {}\n", key, value].as_bytes())
                        .await?;
                }
                self.write.write(b"OK\n").await?;
                Ok(false)
            }
            Err(err) => {
                info!("Cannot handle command: {:?}", err);
                self.write
                    .write(format!["ACK {}\n", err].as_bytes())
                    .await?;
                Ok(false)
            }
        }
    }

    // Tries to executes a command by iterating over the registered handlers.
    // If a handler returns Unsupported, the next one is tried until no more are available.
    async fn exec_command(&mut self, command: Command) -> HandlerResult {
        for handler in self.command_handlers.iter_mut() {
            let (tx, rx) = oneshot::channel();
            handler
                .send(HandlerInput {
                    command: command.clone(),
                    resp: tx,
                })
                .await?;

            let result = rx.await.unwrap();
            match result {
                // Continue in the loop and try next handler
                Err(HandlerError::Unsupported) => (),
                // Otherwise, return result or error
                _ => return result,
            }
        }
        // All handlers returned Unsupported
        Err(HandlerError::Unsupported)
    }
}

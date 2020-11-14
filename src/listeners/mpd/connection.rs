use crate::listeners::mpd::input::read_command;
use crate::listeners::mpd::types::{Handlers, ListenerError};
use crate::mpd_protocol::Command::CommandListStart;
use crate::mpd_protocol::*;
use log::{debug, info, warn};
use tokio::io::{BufReader, Lines};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio::stream::{self, StreamExt};
use tokio::sync::oneshot;

pub static MPD_HELLO_STRING: &[u8] = b"OK MPD 0.21.25\n";

enum OkOutput {
    Ok,
    ListOk,
    None,
}

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
            let ok = match read_command(&mut self.read_lines).await {
                Err(ListenerError::ConnectionClosed) => break,
                Err(err) => self.output_error(err).await,
                Ok(command) => {
                    let result = self.exec_command(command).await;
                    self.output_result(result, OkOutput::Ok).await
                }
            };
            match ok {
                Err(ListenerError::ConnectionClosed) => {
                    break;
                }
                Err(err) => {
                    warn!("Unrecoverable error, closing connection: {}", err);
                    break;
                }
                Ok(()) => {}
            }
        }
    }

    /// Wrapper around exec_one_command to handle command lists
    async fn exec_command(&mut self, command: Command) -> HandlerResult {
        match command {
            // Iterate over command lists
            CommandListStart(list) => {
                for nested in list.get_commands() {
                    match self.exec_one_command(nested).await {
                        Ok(output) => {
                            let ok = if list.is_verbose() {
                                self.output_result(Ok(output), OkOutput::ListOk).await
                            } else {
                                self.output_result(Ok(output), OkOutput::None).await
                            };
                            if let Err(err) = ok {
                                warn!("Cannot print results: {:?}", err);
                            }
                        }
                        Err(err) => return Err(err),
                    }
                }
                Ok(HandlerOutput::Ok)
            }
            // Pass single commands
            _ => self.exec_one_command(command).await,
        }
    }
    /// Tries to executes a command by iterating over the registered handlers.
    /// If a handler returns Unsupported, the next one is tried until no more are available.
    async fn exec_one_command(&mut self, command: Command) -> HandlerResult {
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
        debug!("No handler available to handle {:?}", command);
        Err(HandlerError::Unsupported)
    }

    /// Tries to executes a command by iterating over the registered handlers.
    /// If a handler returns Unsupported, the next one is tried until no more are available.
    async fn output_result(
        &mut self,
        result: HandlerResult,
        ok_output: OkOutput,
    ) -> Result<(), ListenerError> {
        // Unpack handler output by handling error case early
        let output = match result {
            Ok(output) => output,
            Err(err) => return self.output_error(ListenerError::HandlerError(err)).await,
        };

        match output {
            HandlerOutput::Close => {
                debug!("Closing connection due to client command");
                return Err(ListenerError::ConnectionClosed);
            }
            HandlerOutput::Ok => {}
            HandlerOutput::Data(data) => {
                let mut items = stream::iter(data.data);
                while let Some(item) = items.next().await {
                    let bytes = to_vec(item.as_ref())?;
                    if !bytes.is_empty() {
                        self.write.write(bytes.as_ref()).await?;
                        self.write.write(b"\n").await?;
                    }
                }
            }
            HandlerOutput::Fields(data) => {
                for (key, value) in data {
                    self.write
                        .write(format!["{}: {}\n", key, value].as_bytes())
                        .await?;
                }
            }
        }

        match ok_output {
            OkOutput::None => {}
            OkOutput::Ok => {
                self.write.write(b"OK\n").await?;
            }
            OkOutput::ListOk => {
                self.write.write(b"list_OK\n").await?;
            }
        }
        Ok(())
    }

    /// Tries to executes a command by iterating over the registered handlers.
    /// If a handler returns Unsupported, the next one is tried until no more are available.
    async fn output_error(&mut self, err: ListenerError) -> Result<(), ListenerError> {
        info!("Cannot handle command: {:?}", err);
        self.write
            .write(format!["ACK {}\n", err].as_bytes())
            .await?;
        Ok(())
    }
}

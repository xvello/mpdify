use crate::handlers::client::HandlerClient;
use crate::listeners::mpd::idle::{watch_idle, IdleClient};
use crate::listeners::mpd::input::read_command;
use crate::listeners::mpd::types::ListenerError;
use crate::mpd_protocol::Command::CommandListStart;
use crate::mpd_protocol::*;
use crate::util::IdleMessages;
use enumset::EnumSet;
use log::{debug, info, warn};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio_stream::wrappers::LinesStream;
use tokio_stream::{self as stream, StreamExt};

pub static MPD_HELLO_STRING: &[u8] = b"OK MPD 0.21.25\n";

enum OkOutput {
    Ok,
    ListOk,
    None,
}

/// Handles one individual connections and its command flow,
/// run in its own tokio task spawned by MpdListener
pub struct Connection {
    handler: HandlerClient,
    read_lines: LinesStream<BufReader<OwnedReadHalf>>,
    write: OwnedWriteHalf,
    idle_client: IdleClient,
}

impl Connection {
    pub fn new(socket: TcpStream, handler: HandlerClient, idle_messages: IdleMessages) -> Self {
        let (read, write) = socket.into_split();
        let read_lines = LinesStream::new(BufReader::new(read).lines());
        Connection {
            handler,
            read_lines,
            write,
            idle_client: watch_idle(idle_messages),
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
                    warn!("Unrecoverable error, closing connection: {:?}", err);
                    break;
                }
                Ok(()) => {}
            }
        }
    }

    /// Wrapper around exec_one_command to handle command lists
    async fn exec_command(&mut self, command: Command) -> HandlerResult {
        match command {
            // Idle is not supported in a command list
            Command::Idle(subsystems) => self.exec_idle(subsystems).await,
            // Iterate over command lists
            CommandListStart(list) => {
                for nested in list.get_commands() {
                    match self.handler.exec(nested).await {
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
            _ => self.handler.exec(command).await,
        }
    }

    async fn exec_idle(&mut self, subsystems: EnumSet<IdleSubsystem>) -> HandlerResult {
        self.idle_client.start(subsystems).await;
        tokio::select! {
            command = read_command(&mut self.read_lines) => {
                match command {
                    Ok(Command::NoIdle) => {
                        self.idle_client.stop().await;
                        Ok(HandlerOutput::Ok)
                    }
                    Ok(Command::Close) => {
                        Ok(HandlerOutput::Close)
                    }
                    _ => {
                        debug!["Unexpected command {:?} while idle", command];
                        Ok(HandlerOutput::Close)
                    }
                }
            }
            idle = self.idle_client.wait() => {
                Ok(HandlerOutput::Idle(idle))
            }
        }
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
            HandlerOutput::Idle(subsystems) => {
                for subsystem in subsystems {
                    self.write.write(b"changed: ").await?;
                    self.write.write(&to_vec(&subsystem)?).await?;
                    self.write.write(b"\n").await?;
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
        self.write.flush().await?;
        Ok(())
    }

    /// Tries to executes a command by iterating over the registered handlers.
    /// If a handler returns Unsupported, the next one is tried until no more are available.
    async fn output_error(&mut self, err: ListenerError) -> Result<(), ListenerError> {
        info!("Cannot handle command: {:?}", err);
        self.write
            .write(format!["ACK {:?}\n", err].as_bytes())
            .await?;
        Ok(())
    }
}

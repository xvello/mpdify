use crate::mpd::handlers::{HandlerError, HandlerOutput, HandlerResult, HandlerInput};
use tokio::net::{TcpListener, TcpStream};
use log::{debug, info, warn, error};
use crate::mpd::commands::Command;
use std::str::{FromStr, Utf8Error, from_utf8};
use crate::mpd::inputtypes::InputError;
use std::fmt::Debug;
use thiserror::Error;
use tokio::sync::mpsc::{Sender, Receiver};
use tokio::sync::{mpsc, oneshot};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::net::Shutdown;

#[derive(Error, Debug)]
enum ListenerError {
    // Unrecoverable errors that should close the connection
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    UTF(#[from] Utf8Error),

    // Input error that will trigger an ACK but keep the connection open
    #[error(transparent)]
    InputError(#[from] InputError),
    #[error(transparent)]
    HandlerError(#[from] HandlerError),
}

type Handlers = Vec<Sender<HandlerInput>>;

pub struct Listener {
    tcp_listener: TcpListener,
    command_handlers: Handlers,
}

impl Listener {
    pub async fn new(address: String, mut handlers: Handlers) -> Self {
        // Run basic fallback handler
        let (tx, rx) = mpsc::channel(8);
        handlers.push(tx);
        tokio::spawn(async move {
            debug!["starting"];
            BasicCommandHandler::run(rx).await;
        });

        Listener {
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
                Connection{
                    read_buffer: [0; 1024],
                    command_handlers: copied_handlers,
                    socket
                }.run().await;
            });
        }
    }
}

struct Connection {
    read_buffer: [u8; 1024],
    command_handlers: Handlers,
    socket: TcpStream,
}

impl Connection {
    async fn run(&mut self) -> () {
        debug!("New connection");
        loop {
            match self.one().await {
                Ok(closed) => {
                    if closed {
                        break;
                    }
                },
                Err(err) => {
                    warn!("Unrecoverable error, closing connection: {}", err);
                    break;
                },
            }
        }
        // Unconditionally close the tcp connection
        let _ = self.socket.shutdown(Shutdown::Both);
    }

    async fn one(&mut self) -> Result<bool, ListenerError> {
        let n = self.socket.read(&mut self.read_buffer).await?;
        if n == 0 {
            debug!("Client closed the connection");
            return Ok(true);
        }

        let command_string = from_utf8(&self.read_buffer[0..n])?;
        let result = match Command::from_str(command_string) {
            Err(err) => Err(ListenerError::InputError(err)),
            Ok(command) => { match self.exec_command(command).await {
                Err(err) => Err(ListenerError::HandlerError(err)),
                Ok(result) => Ok(result),
            }},
        };

        match result {
            Ok(HandlerOutput::Close) => {
                debug!("Closing connection due to client command");
                Ok(true)
            },
            Ok(HandlerOutput::Ok) => {
                self.socket.write("OK\n".as_bytes()).await?;
                Ok(false)
            },
            Err(err) => {
                info!("Cannot handle command: {:?}", err);
                self.socket.write(format!["ACK {}\n", err].as_bytes()).await?;
                Ok(false)
            }
        }
    }

    // Tries to executes a command by iterating over the registered handlers.
    // If a handler returns Unsupported, the next one is tried until no more are available.
    async fn exec_command(&mut self, command: Command) -> HandlerResult {
        for handler in self.command_handlers.iter_mut() {
            let (tx, rx) = oneshot::channel();
            handler.send(HandlerInput{command: command.clone(), resp: tx }).await?;

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

/// Handles the ping and close commands
pub struct BasicCommandHandler{}

impl BasicCommandHandler {
    async fn run(mut commands: Receiver<HandlerInput>){
        debug!["BasicCommandHandler entered loop"];
        while let Some(input) = commands.recv().await {
            let resp = match input.command {
                Command::Ping => Ok(HandlerOutput::Ok),
                Command::Close => Ok(HandlerOutput::Close),
                _ => Err(HandlerError::Unsupported),
            };
            match input.resp.send(resp) {
                Ok(_) => {},
                Err(err) => {
                    warn!["Cannot send response: {:?}", err];
                },
            }
        }
        debug!["BasicCommandHandler exited loop"];
    }
}

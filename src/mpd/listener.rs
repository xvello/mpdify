use crate::mpd::outputtypes::{CommandHandler, HandlerError, HandlerOutput, HandlerResult};
use std::net::{TcpListener, TcpStream};
use log::{debug, info, warn, error};
use crate::mpd::commands::Command;
use std::str::{FromStr, Utf8Error, from_utf8};
use crate::mpd::inputtypes::InputError;
use std::fmt::Debug;
use thiserror::Error;
use std::io::{Read, Write};
use std::sync::Arc;
use std::{thread, io};

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

pub struct Listener {
    tcp_listener: TcpListener,
    command_handlers: Arc<Vec<Box<dyn CommandHandler>>>,
}

impl Listener {
    pub fn new(address: String) -> Self {
        Listener {
            tcp_listener: TcpListener::bind(address).unwrap(),
            command_handlers: Arc::new(vec![Box::new(BasicCommandHandler{})]),
        }
    }

    pub fn get_address(&self) -> io::Result<String> {
        Ok(self.tcp_listener.local_addr()?.to_string())
    }

    pub fn run(&self) {
        for stream in self.tcp_listener.incoming() {
            let stream = stream.unwrap();
            self.handle_connection(stream);
        }
    }

    fn handle_connection(&self, stream: TcpStream) -> () {
        debug!("New connection");
        let mut handler = ConnectionHandler::new(stream, self.command_handlers.clone());

        // FIXME: use a bounded thread pool
        thread::spawn(move || {
            match handler.listen() {
                Ok(_) => {},
                Err(err) => {
                    warn!("Unrecoverable error: {}", err);
                },
            }
        });
    }
}

struct ConnectionHandler {
    stream: TcpStream,
    read_buffer: [u8; 1024],
    command_handlers: Arc<Vec<Box<dyn CommandHandler>>>,
}

impl ConnectionHandler {
    fn new(stream: TcpStream, command_handlers: Arc<Vec<Box<dyn CommandHandler>>>) -> Self {
        Self {
            stream,
            read_buffer: [0; 1024],
            command_handlers,
        }
    }

    fn listen(&mut self) -> Result<(), ListenerError> {
        // We write full responses, no need to wait for a full packet
        self.stream.set_nodelay(true)?;

        loop {
            match self.one() {
                Ok(close) => {
                    if close {
                        return Ok(());
                    }
                },
                Err(err) => {
                    // Unrecoverable error, log and close connection
                    warn!("Unrecoverable error: {}", err);
                    return Ok(());
                },
            }
        }
    }

    fn one(&mut self) -> Result<bool, ListenerError> {
        let n = self.stream.read(&mut self.read_buffer)?;
        if n == 0 {
            debug!("Client closed the connection");
            return Ok(true);
        }

        let command_string = from_utf8(&self.read_buffer[0..n])?;
        let result = Command::from_str(command_string)
            .map_err(ListenerError::InputError)
            .and_then(|c| self.exec_command(&c).map_err(ListenerError::HandlerError));

        match result {
            Ok(HandlerOutput::Close) => {
                debug!("Closing connection");
                Ok(true)
            },
            Ok(HandlerOutput::Ok) => {
                write!(self.stream, "OK\n")?;
                Ok(false)
            },
            Err(err) => {
                info!("Cannot handle command: {}", err);
                write!(self.stream, "ACK {}\n", err)?;
                Ok(false)
            }
        }
    }

    fn exec_command(&self, command: &Command) -> HandlerResult {
        for handler in self.command_handlers.iter() {
            match handler.handle(command) {
                Ok(result) => return Ok(result),
                Err(err) => {
                    if err != HandlerError::Unsupported {
                        return Err(err);
                    }
                }
            }
        }
        // All handlers returned Unsuported
        Err(HandlerError::Unsupported)
    }
}

/// Handles the ping and close commands
pub struct BasicCommandHandler {}

impl CommandHandler for BasicCommandHandler {
    fn handle(&self, command: &Command) -> HandlerResult {
        match command {
            Command::Ping => Ok(HandlerOutput::Ok),
            Command::Close => Ok(HandlerOutput::Close),
            _ => Err(HandlerError::Unsupported),
        }
    }
}

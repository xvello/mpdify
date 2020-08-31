use crate::mpd::outputtypes::{CommandHandler, HandlerError, HandlerOutput, HandlerResult};
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use log::{debug, info, warn, error};
use crate::mpd::commands::Command;
use std::str::{FromStr, Utf8Error};
use crate::mpd::inputtypes::InputError;
use std::fmt::Debug;
use thiserror::Error;

#[derive(Error, Debug)]
enum ListenerError {
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    UTF(#[from] Utf8Error),
    #[error(transparent)]
    InputError(#[from] InputError),
    #[error(transparent)]
    HandlerError(#[from] HandlerError),
}

pub struct Listener {
    tcp_listener: TcpListener,
    handlers: Vec<Box<dyn CommandHandler>>,
}

impl Listener {
    pub fn new(address: String) -> Self {
        Listener {
            tcp_listener: TcpListener::bind(address).unwrap(),
            handlers: Vec::new(),
        }
    }

    pub fn run(&self) {
        for stream in self.tcp_listener.incoming() {
            let stream = stream.unwrap();
            self.handle_connection(stream);
        }
    }

    fn handle_connection(&self, mut stream: TcpStream) -> () {
        debug!("New connection");
        // We write full responses, no need to wait for a full packet
        stream.set_nodelay(true).unwrap();
        let mut buffer = [0; 1024];

        loop {
            let input = stream.read(&mut buffer)
                .map_err(ListenerError::IO)
                .map(|n| &buffer[0..n])
                .and_then(|b| std::str::from_utf8(b).map_err(ListenerError::UTF));

            match input {
                Ok(s) => {
                    if s.is_empty() {
                        debug!("Client closed the connection");
                        return;
                    }
                },
                Err(err) => {
                    error!("Connection error: {}", err);
                    return;
                },
            }

            match input
                .and_then(|s| Command::from_str(s).map_err(InputError::into))
                .and_then(|c| self.handle(c).map_err(HandlerError::into)) {
                    Ok(HandlerOutput::Close) => {
                        debug!("Closing connection");
                        return;
                    },
                    Ok(HandlerOutput::Ok) => {
                        let r = write!(stream, "OK\n");
                        if r.is_err() {
                            warn!("Failure writing to client: {}", r.err().unwrap());
                            return;
                        }
                    },
                    Err(error) => {
                        info!("Cannot handle command: {}", error);
                        let r= write!(stream, "ACK {}\n", error);
                        if r.is_err() {
                            warn!("Failure writing to client: {}", r.err().unwrap());
                            return;
                        }
                    }
                };
        }
    }
}

impl CommandHandler for Listener {
    fn handle(&self, command: Command) -> HandlerResult {
        match command {
            Command::Ping => Ok(HandlerOutput::Ok),
            Command::Close => Ok(HandlerOutput::Close),
            _ => Err(HandlerError::Unsupported),
        }
    }
}
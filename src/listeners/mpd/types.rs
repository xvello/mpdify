use crate::mpd_protocol::{HandlerError, HandlerInput, InputError};
use std::fmt::Debug;
use thiserror::Error;
use tokio::sync::mpsc::Sender;

#[derive(Error, Debug)]
pub enum ListenerError {
    // Unrecoverable errors that should close the connection
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error("client closed the connection")]
    ConnectionClosed,

    // Input error that will trigger an ACK but keep the connection open
    #[error(transparent)]
    InputError(#[from] InputError),
    #[error(transparent)]
    HandlerError(#[from] HandlerError),
}

pub type Handlers = Vec<Sender<HandlerInput>>;

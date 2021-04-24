use crate::mpd_protocol::{HandlerError, InputError, SerializerError};
use std::fmt::Debug;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ListenerError {
    // Unrecoverable errors that should close the connection
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("client closed the connection")]
    ConnectionClosed,
    #[error(transparent)]
    SerializerError(#[from] SerializerError),
    // Input error that will trigger an ACK but keep the connection open
    #[error(transparent)]
    InputError(#[from] InputError),
    #[error(transparent)]
    HandlerError(#[from] HandlerError),
}

use thiserror::Error;

use crate::mpd::commands::Command;
use tokio::sync::oneshot::Sender;

/// Errors caused by command handling
#[derive(Error, Debug)]
pub enum HandlerError {
    #[error("unsupported operation")]
    Unsupported,
    #[error(transparent)]
    GetError(#[from] tokio::sync::mpsc::error::SendError<HandlerInput>),
    #[error("{0}")]
    FromString(String),
}

/// Commands can return different types of result
#[derive(Debug, PartialEq)]
pub enum HandlerOutput {
    /// Executed OK, no results to return
    Ok,
    /// Executed OK, close the connection
    Close,
}

pub type HandlerResult = Result<HandlerOutput, HandlerError>;

/// Input for the handlers
#[derive(Debug)]
pub struct HandlerInput {
    pub command: Command,
    pub resp: Sender<HandlerResult>,
}

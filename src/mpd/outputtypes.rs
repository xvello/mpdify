use thiserror::Error;

use crate::mpd::commands::Command;

/// Errors caused by command handling
#[derive(Error, Debug, PartialEq)]
pub enum HandlerError {
    #[error("unsupported operation")]
    Unsupported,
}

/// Commands can return different types of result
pub enum HandlerOutput {
    /// Executed OK, no results to return
    Ok,
    /// Executed OK, close the connection
    Close,
}

pub type HandlerResult = Result<HandlerOutput, HandlerError>;

/// Trait the command handlers must implement
pub trait CommandHandler {
    fn handle(&self, command: &Command) -> HandlerResult;
}

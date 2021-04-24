use crate::mpd_protocol::commands::Command;
use crate::mpd_protocol::{IdleSubsystem, OutputData};
use enumset::EnumSet;
use thiserror::Error;
use tokio::sync::oneshot::Sender;

/// Errors caused by command handling
#[derive(Error, Debug)]
pub enum HandlerError {
    #[error("unsupported operation")]
    Unsupported,
    #[error(transparent)]
    GetError(#[from] tokio::sync::mpsc::error::SendError<HandlerInput>),
    #[error("Authenticate at: {0}")]
    AuthNeeded(String),
    #[error(transparent)]
    RedirectedError(#[from] aspotify::RedirectedError),
    #[error("Spotify error: {0}")]
    ASpotifyError(#[from] aspotify::model::Error),
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Cannot retrieve data: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("{0}")]
    FromString(String),
}

/// Commands can return different types of result
#[derive(Debug)]
pub enum HandlerOutput {
    /// Executed OK, no results to return
    Ok,
    /// Executed OK, returns data for client,
    /// as a serializable type
    Data(OutputData),
    /// Executed OK, returns the total size and a
    /// chunk of binary data
    Binary(u64, Vec<u8>),
    /// Executed OK, returns data for client,
    /// as raw lines
    Lines(Vec<String>),
    /// Executed OK, close the connection
    Close,
    /// Result of the idle command,
    Idle(EnumSet<IdleSubsystem>),
}

impl HandlerOutput {
    pub fn from<T: 'static>(value: T) -> HandlerOutput
    where
        T: erased_serde::Serialize + Send,
    {
        HandlerOutput::Data(OutputData::from(value))
    }
}

pub type HandlerResult = Result<HandlerOutput, HandlerError>;

/// Input for the handlers
#[derive(Debug)]
pub struct HandlerInput {
    pub command: Command,
    pub resp: Sender<HandlerResult>,
}

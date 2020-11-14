use serde::export::Formatter;
use serde::Serialize;
use std::fmt;

use crate::mpd_protocol::bool_to_int;

/// Playback status for StatusResponse
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackStatus {
    Play,
    Pause,
    Stop,
}

/// Response for the status command
#[derive(Debug, PartialEq, Serialize)]
pub struct StatusResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<u32>,
    pub state: PlaybackStatus,
    #[serde(serialize_with = "bool_to_int")]
    pub random: bool,
    #[serde(serialize_with = "bool_to_int")]
    pub repeat: bool,
    #[serde(serialize_with = "bool_to_int")]
    pub single: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub playlist_info: Option<StatusPlaylistInfo>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct StatusPlaylistInfo {
    pub playlistlength: usize,
    pub song: usize,
    pub songid: usize,
}

/// Response for the currentsong command
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SongResponse {
    #[serde(rename = "file")]
    pub file: String,
    pub artist: String,
    pub album: String,
    pub title: String,
    pub date: Option<u32>,
    pub pos: usize, // First item of playlist is 0
    pub id: usize,  // First item of playlist is 1
    #[serde(rename = "duration")]
    pub duration: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disc: Option<usize>,
}

/// Holder for HandlerOutput::Serialize
pub struct OutputData {
    pub data: Vec<Box<dyn erased_serde::Serialize + Send>>,
}

impl OutputData {
    pub fn empty() -> OutputData {
        OutputData { data: Vec::new() }
    }

    pub fn from<T: 'static>(value: T) -> OutputData
    where
        T: erased_serde::Serialize + Send,
    {
        let mut out = OutputData::empty();
        out.push(value);
        out
    }

    pub fn push<T: 'static>(&mut self, value: T)
    where
        T: erased_serde::Serialize + Send,
    {
        self.data.push(Box::from(value));
    }
}

impl fmt::Debug for OutputData {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for item in &self.data {
            serde_fmt::to_debug(item.as_ref()).fmt(f)?;
            f.write_str(", ")?;
        }
        Ok(())
    }
}

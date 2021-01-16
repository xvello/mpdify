use serde::{Serialize, Serializer};
use std::fmt;

use crate::mpd_protocol::Path;
use serde::ser::SerializeStruct;
use std::fmt::Formatter;
use std::time::Duration;

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
    pub random: bool,
    pub repeat: bool,
    pub single: bool,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub durations: Option<StatusDurations>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub playlist_info: Option<StatusPlaylistInfo>,
}

#[derive(Debug, PartialEq)]
pub struct StatusDurations {
    pub elapsed: Duration,
    pub duration: Duration,
}

impl Serialize for StatusDurations {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("durations", 3)?;
        let rendered_time = format![
            "{}:{}",
            self.elapsed.as_secs_f64().round(),
            self.duration.as_secs_f64().round()
        ];
        state.serialize_field("time", &rendered_time)?;
        state.serialize_field("elapsed", &self.elapsed.as_secs_f64())?;
        state.serialize_field("duration", &self.duration.as_secs_f64())?;
        state.end()
    }
}

#[derive(Debug, PartialEq, Serialize)]
pub struct StatusPlaylistInfo {
    pub playlistlength: usize,
    pub song: usize,
    pub songid: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nextsong: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nextsongid: Option<usize>,
}

impl StatusPlaylistInfo {
    pub fn new(length: usize, current_pos: usize) -> Self {
        StatusPlaylistInfo {
            playlistlength: length,
            song: current_pos,
            songid: current_pos + 1,
            nextsong: Some(current_pos + 1),
            nextsongid: Some(current_pos + 2),
        }
    }
}

#[derive(Debug, PartialEq, Serialize)]
pub struct VolumeResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<u32>,
}

/// Response for the currentsong command
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SongResponse {
    #[serde(rename = "file")]
    pub file: Path,
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

/// Response for the outputs command
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub struct OutputsResponse {
    pub outputid: usize,
    pub outputname: String,
    pub outputenabled: bool,
    pub plugin: String,
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

impl serde::Serialize for OutputData {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        match self.data.len() {
            // If it holds a single item, silently unpack the list
            1 => self.data.get(0).unwrap().serialize(serializer),
            // Serialize as an item list
            _ => self.data.serialize(serializer),
        }
    }
}

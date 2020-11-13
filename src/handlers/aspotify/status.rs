use crate::mpd_protocol::{HandlerOutput, HandlerResult, PlaybackStatus, StatusResponse};
use aspotify::{CurrentPlayback, PlayingType, RepeatState};
use std::borrow::Borrow;

pub fn build_status_result(input: Option<CurrentPlayback>) -> HandlerResult {
    match input {
        None => Ok(HandlerOutput::from(StatusResponse {
            volume: None,
            state: PlaybackStatus::Stop,
            random: false,
            repeat: false,
            single: false,
            time: None,
            elapsed: None,
            duration: None,
        })),
        Some(data) => Ok(HandlerOutput::from(StatusResponse {
            volume: data.device.volume_percent,
            state: if data.currently_playing.is_playing {
                PlaybackStatus::Play
            } else {
                PlaybackStatus::Pause
            },
            random: data.shuffle_state,
            repeat: RepeatState::Off.ne(data.repeat_state.borrow()),
            single: RepeatState::Track.eq(data.repeat_state.borrow()),
            time: data.currently_playing.progress.map(|d| d.as_secs()),
            elapsed: data.currently_playing.progress.map(|d| d.as_secs_f64()),
            duration: data.currently_playing.item.map(|item| match item {
                PlayingType::Track(track) => track.duration.as_secs_f64(),
                PlayingType::Episode(ep) => ep.duration.as_secs_f64(),
                PlayingType::Ad(ad) => ad.duration.as_secs_f64(),
                PlayingType::Unknown(u) => u.duration.as_secs_f64(),
            }),
        })),
    }
}

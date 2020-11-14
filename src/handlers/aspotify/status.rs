use crate::handlers::aspotify::context::PlayContext;
use crate::handlers::aspotify::song::{build_song_from_episode, build_song_from_track};
use crate::mpd_protocol::{
    HandlerOutput, HandlerResult, PlaybackStatus, StatusPlaylistInfo, StatusResponse,
};
use aspotify::{CurrentPlayback, CurrentlyPlaying, PlayingType, RepeatState};
use std::borrow::Borrow;
use std::sync::Arc;

pub fn build_status_result(
    input: Option<CurrentPlayback>,
    context: Arc<PlayContext>,
) -> HandlerResult {
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
            playlist_info: None,
        })),
        Some(data) => {
            let spotify_id = data
                .currently_playing
                .item
                .as_ref()
                .map(extract_id)
                .flatten()
                .unwrap_or_else(|| String::from("unknown"));
            let pos = context.position_for_id(spotify_id.as_str());
            Ok(HandlerOutput::from(StatusResponse {
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
                playlist_info: Some(StatusPlaylistInfo {
                    playlistlength: context.size(),
                    song: pos,
                    songid: pos + 1,
                }),
            }))
        }
    }
}

pub fn build_song_result(
    input: Option<CurrentlyPlaying>,
    context: Arc<PlayContext>,
) -> HandlerResult {
    input.map_or(Ok(HandlerOutput::Ok), |playing| {
        playing.item.map_or(Ok(HandlerOutput::Ok), |item| {
            let pos_provider = |id: &str| context.position_for_id(id);
            Ok(HandlerOutput::from(match item.borrow() {
                PlayingType::Episode(e) => build_song_from_episode(e, pos_provider),
                PlayingType::Track(t) => build_song_from_track(t, pos_provider),
                PlayingType::Ad(t) => build_song_from_track(t, pos_provider),
                PlayingType::Unknown(t) => build_song_from_track(t, pos_provider),
            }))
        })
    })
}

pub fn extract_id(item: &PlayingType) -> Option<String> {
    match item {
        PlayingType::Track(track) => track.id.clone(),
        PlayingType::Episode(ep) => Some(ep.id.clone()),
        PlayingType::Ad(track) => track.id.clone(),
        PlayingType::Unknown(track) => track.id.clone(),
    }
}

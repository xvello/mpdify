use crate::handlers::aspotify::context::PlayContext;
use crate::mpd_protocol::{
    HandlerOutput, HandlerResult, PlaybackStatus, SongResponse, StatusPlaylistInfo, StatusResponse,
};
use aspotify::{
    ArtistSimplified, CurrentPlayback, CurrentlyPlaying, Episode, PlayingType, RepeatState, Track,
};
use chrono::Datelike;
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
            let pos = context.ordinal_for_id(spotify_id.as_str());
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
        playing
            .item
            .map_or(Ok(HandlerOutput::Ok), |item| match item {
                PlayingType::Episode(e) => build_song_result_from_episode(e, context),
                PlayingType::Track(t) => build_song_result_from_track(t, context),
                PlayingType::Ad(t) => build_song_result_from_track(t, context),
                PlayingType::Unknown(t) => build_song_result_from_track(t, context),
            })
    })
}

pub fn build_song_result_from_track(track: Track, context: Arc<PlayContext>) -> HandlerResult {
    let spotify_id = track.id.unwrap_or_else(|| String::from("unknown"));
    let pos = context.ordinal_for_id(spotify_id.as_str());
    Ok(HandlerOutput::from(SongResponse {
        file: spotify_id,
        artist: flatten_artists(track.artists),
        album: track.album.name,
        title: track.name,
        date: track.album.release_date.map(|d| d.year() as u32),
        pos,
        id: pos + 1,
        duration: track.duration.as_secs_f64(),
        track: Some(track.track_number),
        disc: Some(track.disc_number),
    }))
}

pub fn build_song_result_from_episode(ep: Episode, context: Arc<PlayContext>) -> HandlerResult {
    let spotify_id = ep.id;
    let pos = context.ordinal_for_id(spotify_id.as_str());
    Ok(HandlerOutput::from(SongResponse {
        file: spotify_id,
        artist: ep.show.publisher,
        album: ep.show.name,
        title: ep.name,
        date: Some(ep.release_date.year() as u32),
        pos,
        id: pos + 1,
        duration: ep.duration.as_secs_f64(),
        track: None,
        disc: None,
    }))
}

pub fn extract_id(item: &PlayingType) -> Option<String> {
    match item {
        PlayingType::Track(track) => track.id.clone(),
        PlayingType::Episode(ep) => Some(ep.id.clone()),
        PlayingType::Ad(track) => track.id.clone(),
        PlayingType::Unknown(track) => track.id.clone(),
    }
}

pub fn flatten_artists(artists: Vec<ArtistSimplified>) -> String {
    let mut result = String::new();
    let mut first = true;

    for a in artists {
        result.push_str(a.name.borrow());
        if first {
            first = false;
        } else {
            result.push_str(", ");
        }
    }
    result
}

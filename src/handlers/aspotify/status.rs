use crate::mpd_protocol::{
    HandlerOutput, HandlerResult, PlaybackStatus, SongResponse, StatusResponse,
};
use aspotify::{
    ArtistSimplified, CurrentPlayback, CurrentlyPlaying, Episode, PlayingType, RepeatState, Track,
};
use chrono::Datelike;
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

pub fn build_song_result(input: Option<CurrentlyPlaying>) -> HandlerResult {
    input.map_or(Ok(HandlerOutput::Ok), |playing| {
        playing
            .item
            .map_or(Ok(HandlerOutput::Ok), |item| match item {
                PlayingType::Episode(e) => build_song_result_from_episode(e),
                PlayingType::Track(t) => build_song_result_from_track(t),
                PlayingType::Ad(t) => build_song_result_from_track(t),
                PlayingType::Unknown(t) => build_song_result_from_track(t),
            })
    })
}

pub fn build_song_result_from_track(track: Track) -> HandlerResult {
    Ok(HandlerOutput::from(SongResponse {
        file: track.id.unwrap_or_else(|| String::from("unknown")),
        artist: flatten_artists(track.artists),
        album: track.album.name,
        title: track.name,
        date: track.album.release_date.map(|d| d.year() as u32),
        duration: track.duration.as_secs_f64(),
        track: Some(track.track_number),
        disc: Some(track.disc_number),
    }))
}

pub fn build_song_result_from_episode(ep: Episode) -> HandlerResult {
    Ok(HandlerOutput::from(SongResponse {
        file: ep.id,
        artist: ep.show.publisher,
        album: ep.show.name,
        title: ep.name,
        date: Some(ep.release_date.year() as u32),
        duration: ep.duration.as_secs_f64(),
        track: None,
        disc: None,
    }))
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

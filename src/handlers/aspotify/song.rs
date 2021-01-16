use crate::handlers::aspotify::context::PlayContext;
use crate::mpd_protocol::{HandlerOutput, HandlerResult, Path, SongResponse};
use aspotify::{
    Album, ArtistSimplified, CurrentlyPlaying, Episode, EpisodeSimplified, PlayingType, Show,
    Track, TrackSimplified,
};
use chrono::Datelike;
use std::sync::Arc;

pub fn build_song_from_playing(
    input: Option<&CurrentlyPlaying>,
    context: Arc<PlayContext>,
) -> HandlerResult {
    Ok(match input {
        None => HandlerOutput::Ok,
        Some(input) => match input.item.as_ref() {
            None => HandlerOutput::Ok,
            Some(item) => {
                let pos_provider = |id: &str| context.position_for_id(id);
                HandlerOutput::from(match item {
                    PlayingType::Episode(e) => build_song_from_episode(e, pos_provider),
                    PlayingType::Track(t) => build_song_from_track(t, pos_provider),
                    PlayingType::Ad(t) => build_song_from_track(t, pos_provider),
                    PlayingType::Unknown(t) => build_song_from_track(t, pos_provider),
                })
            }
        },
    })
}
pub fn build_song_from_track(track: &Track, pos_provider: impl Fn(&str) -> usize) -> SongResponse {
    let spotify_id = track.id.clone().unwrap_or_else(String::new);
    let pos = pos_provider(spotify_id.as_str());

    SongResponse {
        file: Path::for_track(unwrap(&track.album.id), unwrap(&track.id)),
        artist: flatten_artists(track.artists.as_ref()),
        album: track.album.name.clone(),
        title: track.name.clone(),
        date: track.album.release_date.map(|d| d.year() as u32),
        pos,
        id: pos + 1,
        duration: track.duration.as_secs_f64(),
        track: Some(track.track_number),
        disc: Some(track.disc_number),
    }
}

pub fn build_song_from_tracksimplified(
    track: &TrackSimplified,
    album: &Album,
    pos: usize,
) -> SongResponse {
    SongResponse {
        file: Path::for_track(&album.id, unwrap(&track.id)),
        artist: flatten_artists(track.artists.as_ref()),
        album: album.name.clone(),
        title: track.name.clone(),
        date: Some(album.release_date.year() as u32),
        pos,
        id: pos + 1,
        duration: track.duration.as_secs_f64(),
        track: Some(track.track_number),
        disc: Some(track.disc_number),
    }
}

pub fn build_song_from_episode(ep: &Episode, pos_provider: impl Fn(&str) -> usize) -> SongResponse {
    let spotify_id = ep.id.as_str();
    let pos = pos_provider(spotify_id);

    SongResponse {
        file: Path::for_episode(&ep.show.id, &ep.id),
        artist: ep.show.publisher.clone(),
        album: ep.show.name.clone(),
        title: ep.name.clone(),
        date: Some(ep.release_date.year() as u32),
        pos,
        id: pos + 1,
        duration: ep.duration.as_secs_f64(),
        track: None,
        disc: None,
    }
}

pub fn build_song_from_episodesimplified(
    ep: &EpisodeSimplified,
    show: &Show,
    pos: usize,
) -> SongResponse {
    SongResponse {
        file: Path::for_episode(&show.id, &ep.id),
        artist: show.publisher.clone(),
        album: show.name.clone(),
        title: ep.name.clone(),
        date: Some(ep.release_date.year() as u32),
        pos,
        id: pos + 1,
        duration: ep.duration.as_secs_f64(),
        track: None,
        disc: None,
    }
}

pub fn flatten_artists(artists: &[ArtistSimplified]) -> String {
    artists
        .iter()
        .map(|a| a.name.as_str())
        .collect::<Vec<&str>>()
        .join(", ")
}

/// Unwrap optional track and album IDs, assuming they are filled
pub fn unwrap(v: &Option<String>) -> &str {
    match v {
        None => "unknown",
        Some(s) => s.as_ref(),
    }
}

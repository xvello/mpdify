use crate::handlers::aspotify::context::PlayContext;
use crate::mpd_protocol::SongResponse;
use aspotify::{Album, ArtistSimplified, Episode, EpisodeSimplified, Show, Track, TrackSimplified};
use chrono::Datelike;
use std::borrow::Borrow;
use std::sync::Arc;

pub fn build_song_from_track(track: &Track, context: Arc<PlayContext>) -> SongResponse {
    let spotify_id = track.id.clone().unwrap_or_else(String::new);
    let pos = context.ordinal_for_id(spotify_id.as_str());

    SongResponse {
        file: spotify_id,
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
    let spotify_id = track.id.clone().unwrap_or_else(String::new);

    SongResponse {
        file: spotify_id,
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

pub fn build_song_from_episode(ep: &Episode, context: Arc<PlayContext>) -> SongResponse {
    let spotify_id = ep.id.as_str();
    let pos = context.ordinal_for_id(spotify_id);

    SongResponse {
        file: spotify_id.to_string(),
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
        file: ep.id.clone(),
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

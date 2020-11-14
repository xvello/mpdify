use crate::handlers::aspotify::context::PlayContext;
use crate::handlers::aspotify::song::{
    build_song_from_episode, build_song_from_episodesimplified, build_song_from_track,
    build_song_from_tracksimplified,
};
use crate::mpd_protocol::{HandlerOutput, HandlerResult, OutputData, PositionRange};
use aspotify::PlaylistItemType;
use std::sync::Arc;

pub fn build_playlistinfo_result(
    context: Arc<PlayContext>,
    range: Option<PositionRange>,
) -> HandlerResult {
    let mut songs = OutputData::empty();
    let range = range.as_ref();
    let include = |pos: usize| -> bool { range.is_none() || range.unwrap().contains(pos) };

    match context.as_ref() {
        PlayContext::Album(album) => {
            for (pos, track) in album.tracks.items.iter().enumerate() {
                if include(pos) {
                    songs.push(build_song_from_tracksimplified(track, album, pos));
                }
            }
        }
        PlayContext::Show(show) => {
            for (pos, ep) in show.episodes.items.iter().enumerate() {
                if include(pos) {
                    songs.push(build_song_from_episodesimplified(ep, show, pos));
                }
            }
        }
        PlayContext::Playlist(playlist) => {
            for (pos, item) in playlist.tracks.items.iter().enumerate() {
                if include(pos) {
                    let pos_provider = |_: &str| pos;
                    match &item.item {
                        PlaylistItemType::Track(track) => {
                            songs.push(build_song_from_track(track, pos_provider))
                        }
                        PlaylistItemType::Episode(ep) => {
                            songs.push(build_song_from_episode(ep, pos_provider))
                        }
                    }
                }
            }
        }
        PlayContext::Track(track) => songs.push(build_song_from_track(track, |_| 0)),
        PlayContext::Episode(ep) => songs.push(build_song_from_episode(ep, |_| 0)),
        PlayContext::Artist(_) => {}
        PlayContext::Empty => {}
    }

    Ok(HandlerOutput::Data(songs))
}

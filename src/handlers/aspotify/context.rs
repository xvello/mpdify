use crate::mpd_protocol::IdleSubsystem;
use crate::util::IdleBus;
use aspotify::Market::FromToken;
use aspotify::{model, Error, ItemType, Track};
use std::borrow::Borrow;
use std::sync::Arc;

#[derive(Debug)]
pub enum PlayContext {
    Album(model::Album),
    Artist(model::Artist, Vec<Track>),
    Playlist(model::Playlist),
    Track(model::Track),
    Show(model::Show),
    Episode(model::Episode),
    Empty,
}

impl PlayContext {
    pub fn size(&self) -> usize {
        match self {
            PlayContext::Album(album) => album.tracks.total,
            PlayContext::Artist(_, tracks) => tracks.len(),
            PlayContext::Playlist(list) => list.tracks.total,
            PlayContext::Track(_) => 1,
            PlayContext::Show(show) => show.episodes.total,
            PlayContext::Episode(_) => 1,
            PlayContext::Empty => 0,
        }
    }

    /// Scans the playing context and returns the position (starting at zero)
    /// of the item with the given ID, if found.
    /// Returns zero if the item is not found.
    /// TODO: we assume IDs are globally unique and don't check the item type (track/episode)
    pub fn position_for_id(&self, id: &str) -> usize {
        match self {
            PlayContext::Album(album) => {
                for (pos, track) in album.tracks.items.iter().enumerate() {
                    if track.id.is_some() && track.id.as_ref().unwrap().eq(id) {
                        return pos;
                    }
                }
            }
            PlayContext::Playlist(playlist) => {
                for (pos, item) in playlist.tracks.items.iter().enumerate() {
                    match &item.item {
                        Some(model::PlaylistItemType::Episode(ep)) => {
                            if ep.id.eq(id) {
                                return pos;
                            }
                        }
                        Some(model::PlaylistItemType::Track(track)) => {
                            if track.id.is_some() && track.id.as_ref().unwrap().eq(id) {
                                return pos;
                            }
                        }
                        None => {}
                    };
                }
            }
            PlayContext::Show(show) => {
                for (pos, item) in show.episodes.items.iter().enumerate() {
                    if item.id.eq(id) {
                        return pos;
                    }
                }
            }
            PlayContext::Artist(_, tracks) => {
                for (pos, track) in tracks.iter().enumerate() {
                    if track.id.is_some() && track.id.as_ref().unwrap().eq(id) {
                        return pos;
                    }
                }
            }
            _ => {}
        };
        // Default to 0 if not found
        0
    }
}

pub struct ContextCache {
    client: Arc<aspotify::Client>,
    idle_bus: Arc<IdleBus>,
    data: Arc<PlayContext>,
    key: Option<model::Context>,
    empty: Arc<PlayContext>,
}

impl ContextCache {
    pub fn new(client: Arc<aspotify::Client>, idle_bus: Arc<IdleBus>) -> ContextCache {
        ContextCache {
            client,
            idle_bus,
            data: Arc::new(PlayContext::Empty),
            key: None,
            empty: Arc::new(PlayContext::Empty),
        }
    }

    pub async fn get(&mut self, key: Option<&model::Context>) -> Result<Arc<PlayContext>, Error> {
        match key {
            None => Ok(self.empty.clone()),
            Some(key) => {
                let hit = self.key.as_ref().map_or(false, |k| k.eq(key));
                if !hit {
                    self.data = Arc::new(self.retrieve(key).await?);
                    self.key = Some(key.clone());
                    self.idle_bus.notify(IdleSubsystem::PlayQueue);
                }
                Ok(self.data.clone())
            }
        }
    }

    pub fn get_latest_key(&self) -> Option<model::Context> {
        self.key.clone()
    }

    async fn retrieve(&mut self, key: &model::Context) -> Result<PlayContext, Error> {
        let id = key.id.borrow();
        Ok(match key.context_type {
            ItemType::Album => {
                let mut album = self.client.albums().get_album(id, None).await?.data;
                while album.tracks.total > album.tracks.items.len() {
                    album.tracks.items.append(
                        &mut self
                            .client
                            .albums()
                            .get_album_tracks(id, 100, album.tracks.items.len(), None)
                            .await?
                            .data
                            .items,
                    );
                }
                PlayContext::Album(album)
            }
            ItemType::Artist => {
                let client = self.client.artists();
                let artist = client.get_artist(id).await?.data;
                let tracks = client.get_artist_top(id, FromToken).await?.data;
                PlayContext::Artist(artist, tracks)
            }
            ItemType::Playlist => {
                let mut playlist = self.client.playlists().get_playlist(id, None).await?.data;
                while playlist.tracks.total > playlist.tracks.items.len() {
                    playlist.tracks.items.append(
                        &mut self
                            .client
                            .playlists()
                            .get_playlists_items(id, 100, playlist.tracks.items.len(), None)
                            .await?
                            .data
                            .items,
                    );
                }
                PlayContext::Playlist(playlist)
            }
            ItemType::Track => {
                PlayContext::Track(self.client.tracks().get_track(id, None).await?.data)
            }
            ItemType::Show => PlayContext::Show(self.client.shows().get_show(id, None).await?.data),
            ItemType::Episode => {
                PlayContext::Episode(self.client.episodes().get_episode(id, None).await?.data)
            }
        })
    }
}

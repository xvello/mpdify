use aspotify::{model, Error, ItemType};
use std::borrow::Borrow;
use std::sync::Arc;

#[derive(Debug)]
pub enum PlayContext {
    Album(model::Album),
    Artist(model::Artist),
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
            PlayContext::Artist(_) => 0, // FIXME
            PlayContext::Playlist(list) => list.tracks.total,
            PlayContext::Track(_) => 1,
            PlayContext::Show(show) => show.episodes.total,
            PlayContext::Episode(_) => 1,
            PlayContext::Empty => 0,
        }
    }

    pub fn ordinal_for_id(&self, id: &str) -> usize {
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
                        model::PlaylistItemType::Episode(ep) => {
                            if ep.id.eq(id) {
                                return pos;
                            }
                        }
                        model::PlaylistItemType::Track(track) => {
                            if track.id.is_some() && track.id.as_ref().unwrap().eq(id) {
                                return pos;
                            }
                        }
                    };
                }
            }
            PlayContext::Artist(_) => {}
            PlayContext::Track(_) => {}
            PlayContext::Show(show) => {
                for (pos, item) in show.episodes.items.iter().enumerate() {
                    if item.id.eq(id) {
                        return pos;
                    }
                }
            }
            PlayContext::Episode(_) => {}
            PlayContext::Empty => {}
        };
        // Default to 0 if not found
        0
    }
}

pub struct ContextCache {
    client: Arc<aspotify::Client>,
    data: Arc<PlayContext>,
    key: Option<model::Context>,
    empty: Arc<PlayContext>,
}

impl ContextCache {
    pub fn new(client: Arc<aspotify::Client>) -> ContextCache {
        ContextCache {
            client,
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
                }
                Ok(self.data.clone())
            }
        }
    }

    async fn retrieve(&mut self, key: &model::Context) -> Result<PlayContext, Error> {
        let id = key.id.borrow();
        Ok(match key.context_type {
            ItemType::Album => {
                PlayContext::Album(self.client.albums().get_album(id, None).await?.data)
            }
            ItemType::Artist => {
                PlayContext::Artist(self.client.artists().get_artist(id).await?.data)
            }
            ItemType::Playlist => {
                PlayContext::Playlist(self.client.playlists().get_playlist(id, None).await?.data)
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

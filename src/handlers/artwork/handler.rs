use crate::handlers::artwork::extract::ExtractArt;
use crate::mpd_protocol::*;
use crate::util::Settings;
use aspotify::Client;
use log::{debug, warn};
use lru_disk_cache::{LruDiskCache, ReadSeek};
use std::ops::Deref;
use std::sync::Arc;
use tokio::io::SeekFrom;
use tokio::sync::mpsc;

pub struct ArtworkHandler {
    command_rx: mpsc::Receiver<HandlerInput>,
    client: Arc<Client>,
    cache: LruDiskCache,
    max_chunk_size: u64,
}

impl ArtworkHandler {
    pub async fn new(
        settings: &Settings,
        client: Arc<Client>,
    ) -> Result<(Self, mpsc::Sender<HandlerInput>), lru_disk_cache::Error> {
        let (command_tx, command_rx) = mpsc::channel(16);
        let cache =
            LruDiskCache::new(settings.artwork_cache_path(), settings.artwork_cache_size())?;
        Ok((
            ArtworkHandler {
                command_rx,
                client,
                cache,
                max_chunk_size: settings.artwork_chunk_size(),
            },
            command_tx,
        ))
    }
    pub async fn run(&mut self) {
        debug!["artwork handler entered loop"];
        // Loop in incoming commands
        while let Some(input) = self.command_rx.recv().await {
            if let Err(err) = input.resp.send(self.execute(input.command).await) {
                warn!["Cannot send response: {:?}", err];
            }
        }
        debug!["artwork handler exited loop"];
    }

    async fn execute(&mut self, command: Command) -> HandlerResult {
        match command {
            Command::AlbumArt(path, offset) => {
                let mut art = self.get_art(path).await?;
                let size = art.seek(SeekFrom::End(0))?;
                let chunk_size = self.max_chunk_size.min(size - offset) as usize;
                let mut data = vec![0; chunk_size];

                art.seek(SeekFrom::Start(offset))?;
                art.read_exact(data.as_mut())?;

                return Ok(HandlerOutput::Binary(size, data));
            }
            _ => Err(HandlerError::Unsupported),
        }
    }

    async fn get_art(&mut self, path: Path) -> Result<Box<dyn ReadSeek>, HandlerError> {
        let (art_id, art_url) = self.resolve_art_url(&path).await?;
        if !self.cache.contains_key(&art_id) {
            let art = reqwest::get(&art_url).await?.bytes().await?;
            self.cache.insert_bytes(&art_id, art.deref())?;
        }
        return self.cache.get(&art_id).map_err(HandlerError::CacheError);
    }

    async fn resolve_art_url(&mut self, path: &Path) -> Result<(String, String), HandlerError> {
        if let Path::Internal(items) = path {
            for (item_type, id) in items.iter().rev() {
                let artwork = match item_type {
                    ItemType::Album => self.client.albums().get_album(id, None).await?.get_art(),
                    ItemType::Show => self.client.shows().get_show(id, None).await?.get_art(),
                    ItemType::Artist => self.client.artists().get_artist(id).await?.get_art(),
                    _ => None,
                };
                if let Some(url) = artwork {
                    return Ok((id.to_string(), url));
                }
            }
        }
        Err(HandlerError::Unsupported)
    }
}

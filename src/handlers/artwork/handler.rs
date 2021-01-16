use crate::mpd_protocol::*;
use crate::util::Settings;
use aspotify::Client;
use hyper::body::Bytes;
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
        if let Some(album_id) = parse_album_id(&path) {
            if !self.cache.contains_key(album_id) {
                let art = self.get_art_for_album(album_id).await?;
                self.cache.insert_bytes(album_id, art.deref())?;
            }
            return self.cache.get(album_id).map_err(HandlerError::CacheError);
        }
        Err(HandlerError::Unsupported)
    }

    async fn get_art_for_album(&mut self, album_id: &str) -> Result<Bytes, HandlerError> {
        log::debug!("Retrieving album info for {}", album_id);
        let album = self.client.albums().get_album(album_id, None).await?;
        match album.data.images.get(0).map(|i| &i.url) {
            None => Err(HandlerError::FromString("No art for album".to_string())),
            Some(url) => {
                log::debug!("Retrieving cover art at {}", url);
                Ok(reqwest::get(url).await?.bytes().await?)
            }
        }
    }
}

fn parse_album_id(path: &Path) -> Option<&str> {
    if let Path::Internal(items) = path {
        for (item_type, id) in items {
            if item_type == &ItemType::Album {
                return Some(id.as_ref());
            }
        }
    }
    None
}

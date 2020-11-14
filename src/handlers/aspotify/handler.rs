use crate::handlers::aspotify::auth::AuthStatus;
use crate::handlers::aspotify::context::ContextCache;
use crate::handlers::aspotify::playlist::build_playlistinfo_result;
use crate::handlers::aspotify::song::build_song_from_playing;
use crate::handlers::aspotify::status::build_status_result;
use crate::mpd_protocol::*;
use crate::util::IdleBus;
use aspotify::{Client, ClientCredentials};
use log::{debug, warn};
use std::sync::Arc;
use tokio::macros::support::Future;
use tokio::sync::mpsc;

pub struct SpotifyHandler {
    command_rx: mpsc::Receiver<HandlerInput>,
    client: Arc<Client>,
    context_cache: ContextCache,
    auth_status: AuthStatus,
}

impl SpotifyHandler {
    pub async fn new(idle_bus: Arc<IdleBus>) -> (Self, mpsc::Sender<HandlerInput>) {
        let (command_tx, command_rx) = mpsc::channel(16);
        let client = Arc::new(Client::new(ClientCredentials::from_env().unwrap()));
        let context_cache = ContextCache::new(client.clone(), idle_bus);
        let auth_status = AuthStatus::new(client.clone()).await;

        (
            SpotifyHandler {
                command_rx,
                client,
                auth_status,
                context_cache,
            },
            command_tx,
        )
    }
    pub async fn run(&mut self) {
        debug!["aspotify handler entered loop"];

        // Loop in incoming commands
        while let Some(input) = self.command_rx.recv().await {
            if let Err(err) = input.resp.send(self.execute(input.command).await) {
                warn!["Cannot send response: {:?}", err];
            }
        }

        debug!["aspotify handler exited loop"];
    }

    async fn execute(&mut self, command: Command) -> HandlerResult {
        let client = self.client.clone();
        match command {
            // Auth support
            Command::SpotifyAuth(token) => match token {
                None => self.auth_status.check().await,
                Some(url) => self.auth_status.callback(url).await,
            },
            // Playback status
            Command::Status => self.execute_status().await,
            Command::CurrentSong => self.execute_currentsong().await,

            // Playback control
            Command::Next => self.exec(client.player().skip_next(None)).await,
            Command::Previous => self.exec(client.player().skip_prev(None)).await,
            Command::Play(None) => self.exec(client.player().resume(None)).await,
            Command::Pause(Some(false)) => self.exec(client.player().resume(None)).await,
            Command::Pause(Some(true)) => self.exec(client.player().pause(None)).await,
            Command::Pause(None) => self.execute_play_pause().await,

            // Volume
            Command::GetVolume => self.execute_get_volume().await,
            Command::SetVolume(value) => {
                self.exec(client.player().set_volume(value as i32, None))
                    .await
            }

            // Playlist info
            Command::PlaylistInfo(range) => self.execute_playlist_info(range).await,
            Command::PlaylistId(id) => match id {
                None => self.execute_playlist_info(None).await,
                Some(id) => {
                    self.execute_playlist_info(Some(PositionRange {
                        start: id - 1,
                        end: id,
                    }))
                    .await
                }
            },

            // Unsupported
            _ => Err(HandlerError::Unsupported),
        }
    }

    async fn exec(
        &mut self,
        f: impl Future<Output = Result<(), aspotify::model::Error>>,
    ) -> HandlerResult where {
        self.auth_status.check().await?;
        f.await?;
        Ok(HandlerOutput::Ok)
    }

    async fn execute_play_pause(&mut self) -> HandlerResult {
        self.auth_status.check().await?;
        match self.client.player().get_playing_track(None).await?.data {
            None => self.client.player().resume(None).await?,
            Some(playing) => match playing.is_playing {
                true => self.client.player().pause(None).await?,
                false => self.client.player().resume(None).await?,
            },
        }
        Ok(HandlerOutput::Ok)
    }

    async fn execute_status(&mut self) -> HandlerResult {
        self.auth_status.check().await?;
        let playback = self.client.player().get_playback(None).await?.data;
        let context_key = playback
            .as_ref()
            .map(|p| p.currently_playing.context.as_ref())
            .flatten();
        let context = self.context_cache.get(context_key).await?;
        build_status_result(playback, context)
    }

    async fn execute_currentsong(&mut self) -> HandlerResult {
        self.auth_status.check().await?;
        let playing = self.client.player().get_playing_track(None).await?.data;
        let context_key = playing.as_ref().map(|p| p.context.as_ref()).flatten();
        let context = self.context_cache.get(context_key).await?;
        build_song_from_playing(playing, context)
    }

    async fn execute_playlist_info(&mut self, range: Option<PositionRange>) -> HandlerResult {
        self.auth_status.check().await?;
        let playing = self.client.player().get_playing_track(None).await?.data;
        let context_key = playing.as_ref().map(|p| p.context.as_ref()).flatten();
        let context = self.context_cache.get(context_key).await?;
        build_playlistinfo_result(playing, context, range)
    }

    async fn execute_get_volume(&mut self) -> HandlerResult {
        self.auth_status.check().await?;
        let playback = self.client.player().get_playback(None).await?.data;
        Ok(HandlerOutput::from(VolumeResponse {
            volume: playback.map(|p| p.device.volume_percent).flatten(),
        }))
    }
}

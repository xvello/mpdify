use crate::handlers::aspotify::auth::AuthStatus;
use crate::handlers::aspotify::context::ContextCache;
use crate::handlers::aspotify::playlist::build_playlistinfo_result;
use crate::handlers::aspotify::song::build_song_from_playing;
use crate::handlers::aspotify::status::build_status_result;
use crate::mpd_protocol::*;
use crate::util::{IdleBus, Settings};
use aspotify::{Client, ClientCredentials, Play};
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

// Alias for aspotify simple return value
type AResult = Result<(), aspotify::model::Error>;

impl SpotifyHandler {
    pub async fn new(
        settings: &Settings,
        idle_bus: Arc<IdleBus>,
    ) -> (Self, mpsc::Sender<HandlerInput>) {
        let (command_tx, command_rx) = mpsc::channel(16);
        let client = Arc::new(Client::new(ClientCredentials::from_env().unwrap()));
        let context_cache = ContextCache::new(client.clone(), idle_bus);
        let auth_status = AuthStatus::new(settings, client.clone()).await;

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
            Command::PlayPos(None) => self.exec(client.player().resume(None)).await,
            Command::PlayPos(Some(pos)) => self.execute_play(pos).await,
            Command::PlayId(None) => self.exec(client.player().resume(None)).await,
            Command::PlayId(Some(0)) => Err(HandlerError::FromString(String::from(
                "songID must be higher and 0",
            ))),
            Command::PlayId(Some(pos)) => self.execute_play(pos - 1).await,
            Command::Pause(Some(false)) => self.exec(client.player().resume(None)).await,
            Command::Pause(Some(true)) => self.exec(client.player().pause(None)).await,
            Command::Pause(None) => self.execute_play_pause().await,
            Command::Stop => self.exec(client.player().pause(None)).await,

            // Volume
            Command::GetVolume => self.execute_get_volume().await,
            Command::ChangeVolume(delta) => self.execute_change_volume(delta).await,
            Command::SetVolume(v) => self.exec(client.player().set_volume(v as i32, None)).await,

            // Playlist info
            Command::PlaylistInfo(range) => self.execute_playlist_info(range).await,
            Command::PlaylistId(None) => self.execute_playlist_info(None).await,
            Command::PlaylistId(Some(0)) => Err(HandlerError::FromString(String::from(
                "songID must be higher and 0",
            ))),
            Command::PlaylistId(Some(id)) => {
                self.execute_playlist_info(Some(PositionRange::one(id - 1)))
                    .await
            }

            // Unsupported
            _ => Err(HandlerError::Unsupported),
        }
    }

    /// Authenticates and executes a simple aspotify call (empty return value).
    async fn exec(&mut self, f: impl Future<Output = AResult>) -> HandlerResult {
        self.auth_status.check().await?;
        f.await?;
        Ok(HandlerOutput::Ok)
    }

    async fn execute_play_pause(&mut self) -> HandlerResult {
        self.auth_status.check().await?;
        let playing = self.client.player().get_playing_track(None).await?.data;
        match playing.map(|p| p.is_playing) {
            None => self.client.player().resume(None).await?,
            Some(false) => self.client.player().resume(None).await?,
            Some(true) => self.client.player().pause(None).await?,
        }
        Ok(HandlerOutput::Ok)
    }

    async fn execute_play(&mut self, pos: usize) -> HandlerResult {
        self.auth_status.check().await?;
        if let Some(context) = self.context_cache.get_latest_key() {
            let target = Play::<'_, &[u8]>::Context(context.context_type, context.id.as_str(), pos);
            self.client.player().play(Some(target), None, None).await?;
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

    async fn get_volume(&mut self) -> Result<Option<u32>, HandlerError> {
        self.auth_status.check().await?;
        let playback = self.client.player().get_playback(None).await?.data;
        Ok(playback.map(|p| p.device.volume_percent).flatten())
    }

    async fn execute_get_volume(&mut self) -> HandlerResult {
        Ok(HandlerOutput::from(VolumeResponse {
            volume: self.get_volume().await?,
        }))
    }

    async fn execute_change_volume(&mut self, delta: i32) -> HandlerResult {
        if let Some(current) = self.get_volume().await? {
            let target = 100.min(0.max(current as i32 + delta));
            self.client.player().set_volume(target, None).await?
        }
        Ok(HandlerOutput::Ok)
    }
}

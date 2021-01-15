use crate::handlers::aspotify::auth::AuthStatus;
use crate::handlers::aspotify::context::ContextCache;
use crate::handlers::aspotify::playback_watcher::PlaybackClient;
use crate::handlers::aspotify::playlist::build_playlistinfo_result;
use crate::handlers::aspotify::song::build_song_from_playing;
use crate::handlers::aspotify::status::{build_outputs_result, build_status_result};
use crate::handlers::aspotify::utils::{compute_repeat, compute_seek};
use crate::mpd_protocol::*;
use crate::util::{IdleBus, Settings};
use aspotify::{Client, Play};
use log::{debug, warn};
use std::sync::Arc;
use std::time::Duration;
use tokio::macros::support::Future;
use tokio::sync::mpsc;

pub struct SpotifyHandler {
    command_rx: mpsc::Receiver<HandlerInput>,
    client: Arc<Client>,
    context_cache: ContextCache,
    auth_status: AuthStatus,
    playback: PlaybackClient,
}

// Alias for aspotify simple return value
type AResult = Result<(), aspotify::model::Error>;

impl SpotifyHandler {
    pub async fn new(
        settings: &Settings,
        client: Arc<Client>,
        idle_bus: Arc<IdleBus>,
    ) -> (Self, mpsc::Sender<HandlerInput>) {
        let (command_tx, command_rx) = mpsc::channel(16);
        let context_cache = ContextCache::new(client.clone(), idle_bus.clone());
        let auth_status = AuthStatus::new(settings, client.clone()).await;
        let playback = PlaybackClient::new(settings, client.clone(), idle_bus);
        (
            SpotifyHandler {
                command_rx,
                client,
                auth_status,
                context_cache,
                playback,
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
            Command::Outputs => self.execute_outputs().await,
            Command::EnableOutput(pos) => self.execute_enable_output(pos).await,

            // Playback options
            Command::Random(state) => self.exec(client.player().set_shuffle(state, None)).await,
            Command::Repeat(state) => self.execute_repeat(Some(state), None).await,
            Command::RepeatSingle(state) => self.execute_repeat(None, Some(state)).await,

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
            Command::SeekCur(time) => self.execute_seek_cur(time).await,
            Command::SeekPos(pos, time) => self.execute_seek(pos, time).await,
            Command::SeekId(0, _) => Err(HandlerError::FromString(String::from(
                "songID must be higher and 0",
            ))),
            Command::SeekId(pos, time) => self.execute_seek(pos - 1, time).await,

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
        self.playback.expect_changes().await;
        Ok(HandlerOutput::Ok)
    }

    async fn execute_play_pause(&mut self) -> HandlerResult {
        self.auth_status.check().await?;
        let playback = self.playback.get().await?;
        match playback.get_playing().map(|p| p.is_playing) {
            None => self.client.player().resume(None).await?,
            Some(false) => self.client.player().resume(None).await?,
            Some(true) => self.client.player().pause(None).await?,
        }
        self.playback.expect_changes().await;
        Ok(HandlerOutput::Ok)
    }

    async fn execute_play(&mut self, pos: usize) -> HandlerResult {
        self.auth_status.check().await?;
        if let Some(context) = self.context_cache.get_latest_key() {
            let target = Play::<'_, &[u8]>::Context(context.context_type, context.id.as_str(), pos);
            self.client.player().play(Some(target), None, None).await?;
        }
        self.playback.expect_changes().await;
        Ok(HandlerOutput::Ok)
    }

    async fn execute_seek_cur(&mut self, time: RelativeFloat) -> HandlerResult {
        self.auth_status.check().await?;
        let elapsed = self.playback.get().await?.get_elapsed();
        self.client
            .player()
            .seek(compute_seek(elapsed, time), None)
            .await?;
        self.playback.expect_changes().await;
        Ok(HandlerOutput::Ok)
    }

    async fn execute_seek(&mut self, pos: usize, time: f64) -> HandlerResult {
        self.auth_status.check().await?;
        let playback = self.playback.get().await?;
        match playback.get_context() {
            None => return Err(HandlerError::FromString("empty playlist".into())),
            Some(context) => {
                let play = Play::<'_, &[u8]>::Context(context.context_type, &context.id, pos);
                self.client
                    .player()
                    .play(Some(play), Some(Duration::from_secs_f64(time)), None)
                    .await?;
            }
        }
        self.playback.expect_changes().await;
        Ok(HandlerOutput::Ok)
    }

    async fn execute_status(&mut self) -> HandlerResult {
        self.auth_status.check().await?;
        let playback = self.playback.get().await?;
        let context = self.context_cache.get(playback.get_context()).await?;
        build_status_result(playback, context)
    }

    async fn execute_outputs(&mut self) -> HandlerResult {
        self.auth_status.check().await?;
        let devices = self.client.player().get_devices().await?;
        build_outputs_result(devices.data)
    }

    async fn execute_enable_output(&mut self, pos: usize) -> HandlerResult {
        self.auth_status.check().await?;
        let devices = self.client.player().get_devices().await?;
        if let Some(Some(dest_id)) = devices.data.get(pos).map(|d| d.id.clone()) {
            self.client.player().transfer(&dest_id, true).await?;
            self.playback.expect_changes().await;
            Ok(HandlerOutput::Ok)
        } else {
            Err(HandlerError::FromString(format!("unknown output: {}", pos)))
        }
    }

    async fn execute_repeat(
        &mut self,
        repeat: Option<bool>,
        single: Option<bool>,
    ) -> HandlerResult {
        self.auth_status.check().await?;
        let playback = self.playback.get().await?;
        if let Some(current) = playback.data.as_ref().map(|d| d.repeat_state) {
            self.client
                .player()
                .set_repeat(compute_repeat(current, repeat, single), None)
                .await?;
            self.playback.expect_changes().await;
        }

        Ok(HandlerOutput::Ok)
    }

    async fn execute_currentsong(&mut self) -> HandlerResult {
        self.auth_status.check().await?;
        let playback = self.playback.get().await?;
        let context = self.context_cache.get(playback.get_context()).await?;
        build_song_from_playing(playback.get_playing(), context)
    }

    async fn execute_playlist_info(&mut self, range: Option<PositionRange>) -> HandlerResult {
        self.auth_status.check().await?;
        let playback = self.playback.get().await?;
        let context = self.context_cache.get(playback.get_context()).await?;
        build_playlistinfo_result(playback.get_playing(), context, range)
    }

    async fn get_volume(&mut self) -> Result<Option<u32>, HandlerError> {
        self.auth_status.check().await?;
        Ok(self
            .playback
            .get()
            .await?
            .data
            .as_ref()
            .map(|d| d.device.volume_percent)
            .flatten())
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
        self.playback.expect_changes().await;
        Ok(HandlerOutput::Ok)
    }
}

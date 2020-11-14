use crate::handlers::aspotify::context::ContextCache;
use crate::handlers::aspotify::playlist::build_playlistinfo_result;
use crate::handlers::aspotify::status::{build_song_result, build_status_result};
use crate::mpd_protocol::*;
use aspotify::{Client, ClientCredentials, Scope};
use log::{debug, warn};
use std::fs;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct SpotifyHandler {
    command_rx: mpsc::Receiver<HandlerInput>,
    client: Arc<Client>,
    auth_state: Option<String>,
    context_cache: ContextCache,
}

static REFRESH_TOKEN_FILE: &str = ".refresh_token";

impl SpotifyHandler {
    pub async fn new() -> (Self, mpsc::Sender<HandlerInput>) {
        let (command_tx, command_rx) = mpsc::channel(16);
        let client = Arc::new(Client::new(ClientCredentials::from_env().unwrap()));
        let context_cache = ContextCache::new(client.clone());

        // Try to read refresh token from file
        if let Ok(token) = fs::read_to_string(REFRESH_TOKEN_FILE) {
            debug!["Reading refresh token from file"];
            client.set_refresh_token(Some(token)).await;
        }

        (
            SpotifyHandler {
                command_rx,
                client,
                auth_state: None,
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
        match command {
            // Auth support
            Command::SpotifyAuth(token) => match token {
                None => self.ensure_authenticated().await,
                Some(url) => self.execute_auth_callback(url).await,
            },
            // Playback status
            Command::Status => self.execute_status().await,
            Command::CurrentSong => self.execute_currentsong().await,
            Command::SetVolume(value) => {
                self.ensure_authenticated().await?;
                self.client.player().set_volume(value as i32, None).await?;
                Ok(HandlerOutput::Ok)
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

    async fn ensure_authenticated(&mut self) -> HandlerResult {
        match self.client.refresh_token().await {
            None => {
                let (url, state) = aspotify::authorization_url(
                    &self.client.credentials.id,
                    vec![
                        Scope::UserReadPlaybackState,
                        Scope::UserModifyPlaybackState,
                        Scope::UserReadCurrentlyPlaying,
                        Scope::Streaming,
                        Scope::AppRemoteControl,
                        Scope::PlaylistReadCollaborative,
                        Scope::PlaylistModifyPublic,
                        Scope::PlaylistReadPrivate,
                        Scope::PlaylistModifyPrivate,
                        Scope::UserLibraryModify,
                        Scope::UserLibraryRead,
                        Scope::UserTopRead,
                        Scope::UserReadRecentlyPlayed,
                        Scope::UserReadPlaybackPosition,
                        Scope::UserFollowRead,
                        Scope::UserFollowModify,
                    ]
                    .iter()
                    .copied(),
                    true,
                    "http://localhost/",
                );
                self.auth_state = Some(state);
                Err(HandlerError::AuthNeeded(url))
            }
            Some(_) => Ok(HandlerOutput::Ok),
        }
    }

    async fn execute_status(&mut self) -> HandlerResult {
        self.ensure_authenticated().await?;
        let playback = self.client.player().get_playback(None).await?.data;
        let context_key = playback
            .as_ref()
            .map(|p| p.currently_playing.context.as_ref())
            .flatten();
        let context = self.context_cache.get(context_key).await?;
        debug!("Context {:?} from key {:?}", context, context_key);
        build_status_result(playback, context)
    }

    async fn execute_currentsong(&mut self) -> HandlerResult {
        self.ensure_authenticated().await?;
        let playing = self.client.player().get_playing_track(None).await?.data;
        let context_key = playing.as_ref().map(|p| p.context.as_ref()).flatten();
        let context = self.context_cache.get(context_key).await?;
        build_song_result(playing, context)
    }

    async fn execute_playlist_info(&mut self, range: Option<PositionRange>) -> HandlerResult {
        self.ensure_authenticated().await?;
        let playing = self.client.player().get_playing_track(None).await?.data;
        let context_key = playing.as_ref().map(|p| p.context.as_ref()).flatten();
        let context = self.context_cache.get(context_key).await?;
        build_playlistinfo_result(context, range)
    }

    async fn execute_auth_callback(&mut self, url: String) -> HandlerResult {
        if self.auth_state.is_none() {
            return Err(HandlerError::FromString("no ongoing auth".to_string()));
        }

        match self
            .client
            .redirected(&url, self.auth_state.as_ref().unwrap())
            .await
        {
            Ok(_) => {
                // Put the refresh token in a file.
                fs::write(
                    REFRESH_TOKEN_FILE,
                    self.client.refresh_token().await.unwrap(),
                )
                .unwrap();

                debug!["Successfully authenticated"];
                Ok(HandlerOutput::Ok)
            }
            Err(err) => {
                debug!["Error authenticating: {:?}", err];
                Err(HandlerError::RedirectedError(err))
            }
        }
    }
}

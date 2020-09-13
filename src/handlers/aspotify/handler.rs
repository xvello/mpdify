use crate::mpd_protocol::*;
use aspotify::{Client, ClientCredentials, Scope};
use log::{debug, warn};
use std::fs;
use tokio::sync::mpsc;

pub struct SpotifyHandler {
    command_rx: mpsc::Receiver<HandlerInput>,
    client: Client,
    auth_state: Option<String>,
}

static REFRESH_TOKEN_FILE: &str = ".refresh_token";

impl SpotifyHandler {
    pub async fn new() -> (Self, mpsc::Sender<HandlerInput>) {
        let (command_tx, command_rx) = mpsc::channel(16);
        let client = Client::new(ClientCredentials::from_env().unwrap());

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
            Command::SetVolume(value) => {
                self.ensure_authenticated().await?;
                self.client.player().set_volume(value as i32, None).await?;
                Ok(HandlerOutput::Ok)
            }

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

        let status = self
            .client
            .player()
            .get_playback(None)
            .await
            .unwrap()
            .data
            .unwrap();

        let mut output: Vec<(String, String)> = vec![];
        if let Some(volume) = status.device.volume_percent {
            output.push(("volume".to_string(), volume.to_string()));
        }
        output.push((
            "state".to_string(),
            if status.currently_playing.is_playing {
                "play".to_string()
            } else {
                "pause".to_string()
            },
        ));
        Ok(HandlerOutput::Data(output))
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

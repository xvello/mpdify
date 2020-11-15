use crate::mpd_protocol::{HandlerError, HandlerOutput, HandlerResult};
use crate::util::Settings;
use aspotify::Scope;
use log::debug;
use std::fs;
use std::sync::Arc;

static REFRESH_TOKEN_FILE: &str = ".refresh_token";

pub struct AuthStatus {
    client: Arc<aspotify::Client>,
    auth_path: String,
    auth_state: Option<String>,
}

impl AuthStatus {
    pub async fn new(settings: &Settings, client: Arc<aspotify::Client>) -> Self {
        // Try to read refresh token from file
        if let Ok(token) = fs::read_to_string(REFRESH_TOKEN_FILE) {
            debug!["Restoring refresh token from file"];
            client.set_refresh_token(Some(token)).await;
        } else {
            debug!["No refresh token found, we will need user input"];
        }

        AuthStatus {
            client,
            auth_path: settings.auth_path(),
            auth_state: None,
        }
    }

    pub async fn check(&mut self) -> HandlerResult {
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
                    self.auth_path.as_str(),
                );
                self.auth_state = Some(state);
                Err(HandlerError::AuthNeeded(url))
            }
            Some(_) => Ok(HandlerOutput::Ok),
        }
    }

    pub async fn callback(&mut self, url: String) -> HandlerResult {
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

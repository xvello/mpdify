use crate::mpd_protocol::{Command, HandlerError, HandlerInput, HandlerResult};
use aspotify::{Client, ClientCredentials};
use std::env::VarError;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

pub fn build_aspotify_client() -> Result<Arc<Client>, VarError> {
    ClientCredentials::from_env().map(Client::new).map(Arc::new)
}

#[derive(Default, Clone)]
pub struct HandlerClient {
    handlers: Vec<mpsc::Sender<HandlerInput>>,
}

impl HandlerClient {
    pub fn new(handlers: Vec<mpsc::Sender<HandlerInput>>) -> Self {
        HandlerClient { handlers }
    }

    pub fn add(&mut self, handler: mpsc::Sender<HandlerInput>) {
        self.handlers.push(handler)
    }

    /// Tries to executes a command by iterating over the registered handlers.
    /// If a handler returns Unsupported, the next one is tried until no more are available.
    pub async fn exec(&self, command: Command) -> HandlerResult {
        for handler in self.handlers.iter() {
            let (tx, rx) = oneshot::channel();
            handler
                .send(HandlerInput {
                    command: command.clone(),
                    resp: tx,
                })
                .await?;

            let result = rx.await.unwrap();
            match result {
                // Continue in the loop and try next handler
                Err(HandlerError::Unsupported) => (),
                // Otherwise, return result or error
                _ => return result,
            }
        }
        // All handlers returned Unsupported
        Err(HandlerError::Unsupported)
    }
}

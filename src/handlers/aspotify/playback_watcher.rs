use crate::handlers::aspotify::playback::CachedPlayback;
use crate::handlers::aspotify::playback_watcher::WatcherCommands::*;
use crate::mpd_protocol::HandlerError;
use crate::util::{IdleBus, Settings};
use aspotify::Response;
use enumset::EnumSet;
use futures::TryFutureExt;
use log::{debug, warn};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::StreamExt;
use tokio_util::time::delay_queue::DelayQueue;

type GetResult = Result<Arc<CachedPlayback>, HandlerError>;

pub struct PlaybackClient {
    tx: mpsc::Sender<WatcherCommands>,
}

impl PlaybackClient {
    pub fn new(settings: &Settings, client: Arc<aspotify::Client>, idle_bus: Arc<IdleBus>) -> Self {
        let (tx, rx) = mpsc::channel(8);
        let mut watcher = PlaybackWatcher::new(settings, client, idle_bus);

        tokio::spawn(async move { watcher.run(rx).await });

        Self { tx }
    }

    pub async fn expect_changes(&mut self) {
        let _ = self.tx.send(WatcherCommands::FastSpeed).await;
    }

    pub async fn get(&mut self) -> GetResult {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .tx
            .send(WatcherCommands::Get(tx))
            .map_err(|e| HandlerError::FromString(e.to_string()))
            .await?;
        rx.await.unwrap()
    }
}

pub enum WatcherCommands {
    FastSpeed,
    SlowSpeed,
    Pool,
    Get(oneshot::Sender<GetResult>),
}

pub struct PlaybackWatcher {
    client: Arc<aspotify::Client>,
    idle_bus: Arc<IdleBus>,
    cache: Arc<CachedPlayback>,
    messages: DelayQueue<WatcherCommands>,
    fast_pool: bool,
    pool_freq_base: Duration,
    pool_freq_fast: Duration,
}

impl PlaybackWatcher {
    pub fn new(settings: &Settings, client: Arc<aspotify::Client>, idle_bus: Arc<IdleBus>) -> Self {
        PlaybackWatcher {
            client,
            idle_bus,
            cache: Arc::new(CachedPlayback::new(None)),
            messages: DelayQueue::new(),
            fast_pool: false,
            pool_freq_base: Duration::from_secs(settings.playback_pool_freq_base_seconds),
            pool_freq_fast: Duration::from_secs(settings.playback_pool_freq_fast_seconds),
        }
    }

    async fn run(&mut self, mut commands_rx: mpsc::Receiver<WatcherCommands>) {
        debug!["playback watcher entered loop"];

        self.messages.insert(Pool, Duration::default());
        loop {
            tokio::select! {
                message = commands_rx.recv() => {
                    if let Some(value) = message {
                        self.on_command(value).await;
                    }
                }
                message = self.messages.next() => {
                    if let Some(Ok(value)) = message {
                        self.on_command(value.into_inner()).await;
                    }
                }
            }
        }
    }

    async fn on_command(&mut self, command: WatcherCommands) {
        match command {
            Pool => self.do_pool().await,
            FastSpeed => {
                self.fast_pool = true;
                self.messages.insert(SlowSpeed, self.pool_freq_base);
                self.messages.insert(Pool, Duration::default());
            }
            SlowSpeed => {
                self.fast_pool = false;
            }
            Get(sender) => {
                if self.cache.data.is_none() {
                    self.do_get().await;
                }
                if sender.send(Ok(self.cache.clone())).is_err() {
                    warn!["Cannot send response"];
                }
            }
        }
    }

    async fn do_pool(&mut self) {
        // Just empty the cache if we don't have active clients
        if !self.idle_bus.has_subscribers() {
            debug!("No client listening, skipping pool");
            self.clear_cache();
            if self.fast_pool {
                self.messages.insert(Pool, self.pool_freq_fast);
            } else {
                self.messages.insert(Pool, self.pool_freq_base);
            }
            return;
        }

        self.do_get().await;

        if self.fast_pool {
            self.messages.insert(Pool, self.pool_freq_fast);
        } else {
            self.messages.insert(Pool, self.pool_freq_base);
        }
    }

    fn clear_cache(&mut self) {
        if self.cache.data.is_some() {
            self.cache = CachedPlayback::new(None).into();
        }
    }

    async fn do_get(&mut self) {
        debug!("Retrieving status...");
        let changed = match self.client.player().get_playback(None).await {
            Err(err) => {
                warn!("Error fetching playback state: {}", err);
                EnumSet::empty()
            }
            Ok(Response { data: new, .. }) => {
                let changed = self.cache.compare(&new);
                if !changed.is_empty() {
                    self.cache = CachedPlayback::new(new).into();
                }
                changed
            }
        };

        if !changed.is_empty() {
            debug!("Detected changes: {:?}", changed);
            self.fast_pool = false;
            for s in changed {
                self.idle_bus.notify(s)
            }
        } else {
            debug!("Detected no changes");
        }
    }
}

use crate::mpd_protocol::IdleSubsystem;
use crate::util::IdleMessages;
use enumset::EnumSet;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

pub struct IdleClient {
    watch_rx: mpsc::Receiver<EnumSet<IdleSubsystem>>,
    enable_tx: mpsc::Sender<EnumSet<IdleSubsystem>>,
}

impl IdleClient {
    pub async fn start(&mut self, subsystems: EnumSet<IdleSubsystem>) {
        let _ = self.enable_tx.send(subsystems).await;
    }

    pub async fn stop(&mut self) {
        let _ = self.enable_tx.send(EnumSet::empty()).await;
    }

    pub async fn wait(&mut self) -> EnumSet<IdleSubsystem> {
        self.watch_rx.recv().await.unwrap_or_default()
    }
}

struct WatcherState {
    changed: EnumSet<IdleSubsystem>,
    waiting: EnumSet<IdleSubsystem>,
    watch_tx: mpsc::Sender<EnumSet<IdleSubsystem>>,
    send_err: bool,
}

impl WatcherState {
    async fn run(
        &mut self,
        mut messages: IdleMessages,
        mut enable_rx: mpsc::Receiver<EnumSet<IdleSubsystem>>,
    ) {
        loop {
            tokio::select! {
                message = messages.recv() => {
                    if let Ok(message) = message {
                        self.changed.insert(message.what);

                        // Wait 50ms for other messages to aggregate
                        while let Ok(Ok(message)) = timeout(Duration::from_millis(50), messages.recv()).await {
                            self.changed.insert(message.what);
                        }

                        self.check().await;
                    } else {
                        break;
                    }
                }
                enable = enable_rx.recv() => {
                    if let Some(enable) = enable {
                        self.waiting = enable;
                        self.check().await;
                    } else {
                        break;
                    }
                }
            }
            if self.send_err {
                break;
            }
        }
    }

    async fn check(&mut self) {
        if !self.changed.is_disjoint(self.waiting) {
            let matching = self.changed.intersection(self.waiting);
            self.changed.remove_all(self.waiting); // FIXME: do we want to clear instead?
            self.waiting = EnumSet::empty();

            self.send_err = self.watch_tx.send(matching).await.is_err();
        }
    }
}

pub fn watch_idle(messages: IdleMessages) -> IdleClient {
    let (watch_tx, watch_rx) = mpsc::channel(8);
    let (enable_tx, enable_rx) = mpsc::channel(8);

    let mut state = WatcherState {
        changed: EnumSet::new(),
        waiting: EnumSet::empty(),
        watch_tx,
        send_err: false,
    };

    tokio::spawn(async move { state.run(messages, enable_rx).await });

    IdleClient {
        watch_rx,
        enable_tx,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mpd_protocol::IdleSubsystem::{Mixer, PlayQueue, Player};
    use crate::util::IdleBus;
    use std::sync::Arc;
    use tokio::time::{timeout, Duration};

    fn setup() -> (Arc<IdleBus>, IdleClient) {
        let _ = pretty_env_logger::try_init();
        let bus = IdleBus::new();
        let client = watch_idle(bus.subscribe());
        (bus, client)
    }

    async fn assert_receive(client: &mut IdleClient, expected: EnumSet<IdleSubsystem>) {
        let output = timeout(Duration::from_millis(250), client.wait())
            .await
            .expect("No notification received");
        assert_eq!(output, expected);
    }

    async fn assert_nothing(client: &mut IdleClient) {
        let output = timeout(Duration::from_millis(50), client.wait()).await;
        assert!(output.is_err(), "Unexpected notification received");
    }

    #[tokio::test]
    async fn test_it_matches_one_subsystem() {
        let (bus, mut watcher) = setup();
        watcher.start(EnumSet::only(Player)).await;
        assert_nothing(&mut watcher).await;

        bus.notify(Player);
        assert_receive(&mut watcher, EnumSet::only(Player)).await;
    }

    #[tokio::test]
    async fn test_it_matches_two_subsystems() {
        let (bus, mut watcher) = setup();
        watcher.start(Player | Mixer).await;
        assert_nothing(&mut watcher).await;

        bus.notify(Player);
        bus.notify(Mixer);
        bus.notify(PlayQueue);
        assert_receive(&mut watcher, Player | Mixer).await;

        // PlayQueue notif is still queued
        watcher.start(EnumSet::only(PlayQueue)).await;
        assert_receive(&mut watcher, EnumSet::only(PlayQueue)).await;
    }

    #[tokio::test]
    async fn test_it_watches_since_creation() {
        let (bus, mut watcher) = setup();
        bus.notify(Player);
        assert_nothing(&mut watcher).await;

        watcher.start(EnumSet::only(Player)).await;
        assert_receive(&mut watcher, EnumSet::only(Player)).await;
    }

    #[tokio::test]
    async fn test_it_stops_after_match() {
        let (bus, mut watcher) = setup();
        watcher.start(EnumSet::only(Player)).await;
        assert_nothing(&mut watcher).await;

        // We get the first notification
        bus.notify(Player);
        assert_receive(&mut watcher, EnumSet::only(Player)).await;

        // Watcher has stopped automatically
        bus.notify(Player);
        assert_nothing(&mut watcher).await;

        // Notification was queued
        watcher.start(EnumSet::only(Player)).await;
        assert_receive(&mut watcher, EnumSet::only(Player)).await;
    }

    #[tokio::test]
    async fn test_it_can_stop_and_restart() {
        let (bus, mut watcher) = setup();
        watcher.start(EnumSet::only(Player)).await;
        assert_nothing(&mut watcher).await;

        // Watcher is stopped before the notification
        watcher.stop().await;
        assert_nothing(&mut watcher).await;
        bus.notify(Player);
        assert_nothing(&mut watcher).await;

        // Notification was queued
        watcher.start(EnumSet::only(Player)).await;
        assert_receive(&mut watcher, EnumSet::only(Player)).await;
    }

    #[tokio::test]
    async fn test_it_remembers_other_subsystem() {
        let (bus, mut watcher) = setup();
        watcher.start(EnumSet::only(Player)).await;
        assert_nothing(&mut watcher).await;

        // First subsystem triggers a notif, the other is queued
        bus.notify(Player);
        bus.notify(Mixer);
        assert_receive(&mut watcher, EnumSet::only(Player)).await;

        // Player subsystem has no new changes
        watcher.start(EnumSet::only(Player)).await;
        assert_nothing(&mut watcher).await;

        // Mixer subsystem has one
        watcher.start(EnumSet::only(Mixer)).await;
        assert_receive(&mut watcher, EnumSet::only(Mixer)).await;
    }
}

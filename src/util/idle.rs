use crate::mpd_protocol::IdleSubsystem;
use log::debug;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;

pub type IdleMessages = broadcast::Receiver<IdleMessage>;

#[derive(Debug, Copy, Clone)]
pub struct IdleMessage {
    pub what: IdleSubsystem,
    pub when: Instant,
}

pub struct IdleBus {
    channel: broadcast::Sender<IdleMessage>,
}

impl IdleBus {
    #[must_use]
    pub fn new() -> Arc<IdleBus> {
        let (channel, _) = broadcast::channel(16);
        Arc::new(IdleBus { channel })
    }

    /// Returns a channel for notifications, that can be safely dropped
    pub fn subscribe(&self) -> IdleMessages {
        self.channel.subscribe()
    }

    /// Returns true if at least one client is subscribed to updates
    pub fn has_subscribers(&self) -> bool {
        self.channel.receiver_count() > 0
    }

    /// Send a notification with the current timestamp,
    /// ignores channel errors caused by no subscriber
    pub fn notify(&self, system: IdleSubsystem) {
        debug!["Notifying change in {:?}", system];
        let _ = self.channel.send(IdleMessage {
            what: system,
            when: Instant::now(),
        });
    }
}

use crate::handlers::mpris::MEDIAPLAYER2_PATH;
use crate::mpd_protocol::IdleSubsystem;
use crate::util::IdleBus;
use dbus::message::SignalArgs;
use dbus::nonblock::stdintf::org_freedesktop_dbus::PropertiesPropertiesChanged;
use dbus::nonblock::{MsgMatch, SyncConnection};
use dbus::strings::BusName;
use dbus::Error;
use dbus::Path;
use log::debug;
use std::borrow::Borrow;
use std::sync::Arc;
use std::time::{Duration, Instant};

static SIGNAL_MIN_INTERVAL: Duration = Duration::from_millis(250);

pub struct MprisWatcher {
    conn: Arc<SyncConnection>,
    matcher: MsgMatch,
}

impl MprisWatcher {
    pub async fn new(
        conn: Arc<SyncConnection>,
        idle_bus: Arc<IdleBus>,
        target_name: &str,
    ) -> Result<Self, Error> {
        let idle_path = Path::from(MEDIAPLAYER2_PATH);
        let idle_busname = BusName::from(target_name);
        let idle_mc = PropertiesPropertiesChanged::match_rule(
            Some(idle_busname.borrow()),
            Some(idle_path.borrow()),
        );

        // Debounce messages and transmit them to subscribers
        let mut last_send = Instant::now();
        let matcher =
            conn.add_match(idle_mc.static_clone())
                .await?
                .cb(move |_, (_,): (String,)| {
                    if last_send.elapsed().ge(&SIGNAL_MIN_INTERVAL) {
                        // FIXME: we don't have enough info to know what changed
                        idle_bus.notify(IdleSubsystem::Player);
                        idle_bus.notify(IdleSubsystem::Mixer);
                        idle_bus.notify(IdleSubsystem::Options);
                        last_send = Instant::now();
                    }
                    true
                });

        debug!["Watching for property changes on dbus"];
        Ok(Self { conn, matcher })
    }

    pub async fn close(&self) -> Result<(), Error> {
        debug!["Stop watching for property changes on dbus"];
        Ok(self.conn.remove_match(self.matcher.token()).await?)
    }
}

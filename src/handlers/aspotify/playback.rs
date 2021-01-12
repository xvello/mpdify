use crate::mpd_protocol::IdleSubsystem;
use aspotify::{model, CurrentPlayback};
use enumset::EnumSet;
use std::borrow::Borrow;
use std::time::{Duration, Instant};

pub struct CachedPlayback {
    pub data: Option<CurrentPlayback>,
    retrieved: Instant,
}

impl CachedPlayback {
    pub fn new(playback: Option<CurrentPlayback>) -> Self {
        CachedPlayback {
            data: playback,
            retrieved: Instant::now(),
        }
    }

    pub fn get_context(&self) -> Option<&model::Context> {
        self.data
            .as_ref()
            .map(|d| d.currently_playing.context.as_ref())
            .flatten()
    }

    pub fn get_playing(&self) -> Option<&model::CurrentlyPlaying> {
        self.data.as_ref().map(|d| d.currently_playing.borrow())
    }

    pub fn get_elapsed(&self) -> Option<Duration> {
        match &self.data {
            None => None,
            Some(playing) => {
                let progress = playing.currently_playing.progress;
                if playing.currently_playing.is_playing {
                    let progress_delta = Instant::now().duration_since(self.retrieved);
                    progress.map(|e| e + progress_delta)
                } else {
                    progress
                }
            }
        }
    }

    pub fn compare(&self, other: &Option<CurrentPlayback>) -> EnumSet<IdleSubsystem> {
        match &self.data {
            None => match other {
                None => EnumSet::empty(),
                Some(_) => EnumSet::all(),
            },
            Some(old) => match other {
                None => EnumSet::only(IdleSubsystem::Player),
                Some(new) => {
                    let mut changed = EnumSet::new();
                    if old.shuffle_state != new.shuffle_state {
                        changed.insert(IdleSubsystem::Options);
                    }
                    if old.repeat_state != new.repeat_state {
                        changed.insert(IdleSubsystem::Options);
                    }
                    if old.device.volume_percent != new.device.volume_percent {
                        changed.insert(IdleSubsystem::Mixer);
                    }
                    if old.currently_playing.is_playing != new.currently_playing.is_playing {
                        changed.insert(IdleSubsystem::Player);
                    }
                    if old.currently_playing.context != new.currently_playing.context {
                        changed.insert(IdleSubsystem::PlayQueue);
                    }
                    if old.currently_playing.item != new.currently_playing.item {
                        changed.insert(IdleSubsystem::Player);
                    }
                    if old.device.name != new.device.name {
                        changed.insert(IdleSubsystem::Outputs);
                    }
                    if CachedPlayback::detect_seek(
                        self.get_elapsed(),
                        new.currently_playing.progress,
                    ) {
                        changed.insert(IdleSubsystem::Player);
                    }
                    changed
                }
            },
        }
    }

    /// Detect whether the player was seeked, with half a second of tolerance
    fn detect_seek(old: Option<Duration>, new: Option<Duration>) -> bool {
        let delta = old.unwrap_or_default().as_secs_f64() - new.unwrap_or_default().as_secs_f64();
        delta.abs() > 0.5
    }
}

#[cfg(test)]
mod tests {
    use crate::handlers::aspotify::playback::CachedPlayback;
    use crate::mpd_protocol::IdleSubsystem;
    use aspotify::{Actions, CurrentPlayback, CurrentlyPlaying, Device, DeviceType, RepeatState};
    use enumset::EnumSet;
    use std::time::{Duration, Instant};

    const PLAYED_SECONDS: u64 = 90;
    const DELTA_SECONDS: u64 = 12;

    fn build_current_playback(
        progress: Option<Duration>,
        is_playing: bool,
        retrieved: Instant,
    ) -> CachedPlayback {
        CachedPlayback {
            data: Some(CurrentPlayback {
                device: Device {
                    id: None,
                    is_active: false,
                    is_private_session: false,
                    is_restricted: false,
                    name: "".to_string(),
                    device_type: DeviceType::Computer,
                    volume_percent: Some(20),
                },
                repeat_state: RepeatState::Off,
                shuffle_state: false,
                currently_playing: CurrentlyPlaying {
                    context: None,
                    progress,
                    is_playing,
                    item: None,
                    actions: Actions { disallows: vec![] },
                },
            }),
            retrieved,
        }
    }

    fn assert_properties(status: CachedPlayback, elapsed: u64, is_playing: bool) {
        assert_eq!(elapsed, status.get_elapsed().unwrap().as_secs());
        let playback = status.data.unwrap();
        assert_eq!(is_playing, playback.currently_playing.is_playing);
        assert_eq!(Some(20), playback.device.volume_percent);
    }

    fn assert_changes(old: CachedPlayback, new: CachedPlayback, expected: Vec<IdleSubsystem>) {
        let mut expected_set = EnumSet::new();
        for e in expected {
            expected_set.insert(e);
        }
        assert_eq!(expected_set, old.compare(&new.data));
    }

    #[test]
    fn it_returns_paused_playback_as_is() {
        let p = build_current_playback(
            Some(Duration::from_secs(PLAYED_SECONDS)),
            false,
            Instant::now() - Duration::from_secs(DELTA_SECONDS),
        );
        assert_properties(p, PLAYED_SECONDS, false)
    }

    #[test]
    fn it_returns_fresh_playback_as_is() {
        let p = build_current_playback(
            Some(Duration::from_secs(PLAYED_SECONDS)),
            true,
            Instant::now(),
        );
        assert_properties(p, PLAYED_SECONDS, true)
    }

    #[test]
    fn it_updates_playing_playback() {
        let p = build_current_playback(
            Some(Duration::from_secs(PLAYED_SECONDS)),
            true,
            Instant::now() - Duration::from_secs(DELTA_SECONDS),
        );
        assert_properties(p, PLAYED_SECONDS + DELTA_SECONDS, true)
    }

    #[test]
    fn it_detects_seek() {
        let p1 = build_current_playback(
            Some(Duration::from_secs(PLAYED_SECONDS)),
            true,
            Instant::now() - Duration::from_secs(DELTA_SECONDS),
        );
        let p2 = build_current_playback(
            Some(Duration::from_secs(PLAYED_SECONDS)),
            true,
            Instant::now(),
        );
        assert_changes(p1, p2, vec![IdleSubsystem::Player])
    }

    #[test]
    fn it_ignores_small_seek() {
        let p1 = build_current_playback(
            Some(Duration::from_secs(PLAYED_SECONDS) - Duration::from_millis(400)),
            true,
            Instant::now(),
        );
        let p2 = build_current_playback(
            Some(Duration::from_secs(PLAYED_SECONDS)),
            true,
            Instant::now(),
        );
        assert_changes(p1, p2, vec![])
    }

    #[test]
    fn it_detects_no_seek_when_paused() {
        let p1 = build_current_playback(
            Some(Duration::from_secs(PLAYED_SECONDS)),
            false,
            Instant::now() - Duration::from_secs(DELTA_SECONDS),
        );
        let p2 = build_current_playback(
            Some(Duration::from_secs(PLAYED_SECONDS)),
            false,
            Instant::now(),
        );
        assert_changes(p1, p2, vec![])
    }
}

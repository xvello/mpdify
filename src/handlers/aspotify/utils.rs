use crate::mpd_protocol::RelativeFloat;
use aspotify::RepeatState;
use std::time::Duration;

pub fn compute_seek(current: Option<Duration>, seek: RelativeFloat) -> Duration {
    match seek {
        RelativeFloat::Absolute(time) => Duration::from_secs_f64(time),
        RelativeFloat::Relative(delta) => {
            let delta_duration = Duration::from_secs_f64(delta.abs());
            if delta.is_sign_positive() {
                current.map(|d| d.checked_add(delta_duration))
            } else {
                current.map(|d| d.checked_sub(delta_duration))
            }
            .flatten()
            .unwrap_or_else(Duration::default)
        }
    }
}

pub fn compute_repeat(
    current: RepeatState,
    repeat: Option<bool>,
    single: Option<bool>,
) -> RepeatState {
    let desired_repeat = repeat.unwrap_or(current != RepeatState::Off);
    let desired_single = single.unwrap_or(current == RepeatState::Track);
    match desired_repeat {
        false => RepeatState::Off,
        true => match desired_single {
            false => RepeatState::Context,
            true => RepeatState::Track,
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::handlers::aspotify::time::{compute_repeat, compute_seek};
    use crate::mpd_protocol::RelativeFloat::{Absolute, Relative};
    use aspotify::RepeatState::{Context, Off, Track};
    use std::time::Duration;

    #[test]
    fn it_returns_absolute_time() {
        assert_eq!(50, compute_seek(None, Absolute(50.)).as_secs());
        assert_eq!(
            50,
            compute_seek(Some(Duration::from_secs(20)), Absolute(50.)).as_secs()
        )
    }

    #[test]
    fn it_goes_forward_on_current() {
        assert_eq!(
            70,
            compute_seek(Some(Duration::from_secs(20)), Relative(50.)).as_secs()
        )
    }

    #[test]
    fn it_does_not_seek_on_none() {
        assert_eq!(0, compute_seek(None, Relative(50.)).as_secs())
    }

    #[test]
    fn it_goes_backwards() {
        assert_eq!(
            40,
            compute_seek(Some(Duration::from_secs(90)), Relative(-50.)).as_secs()
        )
    }

    #[test]
    fn it_goes_backwards_until_zero() {
        assert_eq!(
            0,
            compute_seek(Some(Duration::from_secs(9)), Relative(-50.)).as_secs()
        )
    }

    #[test]
    fn it_computes_desired_repeat() {
        let cases = vec![
            (Off, Some(true), None, Context),
            (Off, Some(true), Some(true), Track),
            (Track, Some(false), Some(true), Off),
            (Off, Some(false), Some(true), Off),
            (Track, None, Some(true), Track),
            (Track, None, Some(false), Context),
        ];
        for (current, repeat, single, expected) in cases {
            assert_eq!(expected, compute_repeat(current, repeat, single));
        }
    }
}

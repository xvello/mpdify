use crate::mpd_protocol::RelativeFloat;
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

#[cfg(test)]
mod tests {
    use crate::handlers::aspotify::time::compute_seek;
    use crate::mpd_protocol::RelativeFloat::{Absolute, Relative};
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
}

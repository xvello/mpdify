use crate::mpd_protocol::HandlerError;
use serde::{ser, Serializer};

/// FIXME: should be transparent via a custom Serializer implem
pub fn bool_to_int<S>(value: &bool, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_u8(*value as u8)
}

/// Currently piggy-backing on the yaml serializer, just removing the first `---\n` line
pub fn to_vec<T: ?Sized>(value: &T) -> Result<Vec<u8>, HandlerError>
where
    T: ser::Serialize,
{
    let mut yaml =
        serde_yaml::to_vec(value).map_err(|err| HandlerError::FromString(err.to_string()))?;
    yaml.drain(0..4);
    Ok(yaml)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mpd_protocol::{PlaybackStatus, StatusResponse};

    #[test]
    fn test_string() {
        assert_eq!(
            to_vec(&String::from("hello")).expect("Serializer error"),
            b"hello".to_vec()
        );
    }

    #[test]
    fn test_status() {
        assert_eq!(
            to_vec(&StatusResponse {
                volume: None,
                state: PlaybackStatus::Stop,
                random: false,
                repeat: false,
                single: false,
                time: None,
                elapsed: None,
                duration: None,
            })
            .expect("Serializer error"),
            b"state: stop\nrandom: 0\nrepeat: 0\nsingle: 0".to_vec()
        );

        assert_eq!(
            to_vec(&StatusResponse {
                volume: Some(20),
                state: PlaybackStatus::Pause,
                random: true,
                repeat: false,
                single: false,
                time: None,
                elapsed: None,
                duration: Some(120.6),
            })
            .expect("Serializer error"),
            b"volume: 20\nstate: pause\nrandom: 1\nrepeat: 0\nsingle: 0\nduration: 120.6".to_vec()
        );
    }
}

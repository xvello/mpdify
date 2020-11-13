use crate::mpd_protocol::HandlerError;
use serde::ser;

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
                state: PlaybackStatus::Stop
            })
            .expect("Serializer error"),
            b"state: stop"
        );

        assert_eq!(
            to_vec(&StatusResponse {
                volume: Some(20),
                state: PlaybackStatus::Pause
            })
            .expect("Serializer error"),
            b"volume: 20\nstate: pause"
        );
    }
}

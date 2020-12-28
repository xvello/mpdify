use crate::mpd_protocol::HandlerError;
use lazy_static::lazy_static;
use regex::Regex;
use serde::{ser, Serializer};

/// FIXME: should be transparent via a custom Serializer implem
pub fn bool_to_int<S>(value: &bool, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_u8(*value as u8)
}

/// FIXME: write ad-hoc serialization
/// Currently piggy-backing on the yaml serializer, with highly inefficient string manipulation hacks
pub fn to_vec<T: ?Sized>(value: &T) -> Result<Vec<u8>, HandlerError>
where
    T: ser::Serialize,
{
    let mut yaml =
        serde_yaml::to_string(value).map_err(|err| HandlerError::FromString(err.to_string()))?;
    yaml.drain(0..4);

    // FIXME: hack to mute empty yaml objects
    if yaml == "{}" {
        return Ok(vec![]);
    }
    // FIXME: hack to unquote the status time field
    lazy_static! {
        static ref UNQUOTE_TIME_RE: Regex = Regex::new(r#"(?m)^time: "(\d+:\d+)"$"#).unwrap();
    }
    let patched = UNQUOTE_TIME_RE.replace(&yaml, r"time: $1");

    Ok(patched.as_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mpd_protocol::{PlaybackStatus, StatusDurations, VolumeResponse};
    use serde::Serialize;
    use std::time::Duration;

    #[derive(Debug, PartialEq, Serialize)]
    #[serde(rename_all = "PascalCase")]
    pub struct CustomResponse {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub optional_int: Option<u32>,
        pub status: PlaybackStatus,
        #[serde(rename = "float")]
        pub float: f64,
    }

    #[test]
    fn test_string() {
        assert_eq!(
            to_vec(&String::from("hello")).expect("Serializer error"),
            b"hello".to_vec()
        );
    }

    #[test]
    fn test_empty() {
        assert_eq!(
            to_vec(&VolumeResponse { volume: None }).expect("Serializer error"),
            b"".to_vec()
        );
    }

    #[test]
    fn test_status() {
        assert_eq!(
            to_vec(&CustomResponse {
                optional_int: None,
                status: PlaybackStatus::Stop,
                float: 7.0,
            })
            .expect("Serializer error"),
            b"Status: stop\nfloat: 7.0".to_vec()
        );

        assert_eq!(
            to_vec(&CustomResponse {
                optional_int: Some(20),
                status: PlaybackStatus::Pause,
                float: 6.66
            })
            .expect("Serializer error"),
            b"OptionalInt: 20\nStatus: pause\nfloat: 6.66".to_vec()
        );
    }

    #[test]
    fn test_durations() {
        assert_eq!(
            std::str::from_utf8(
                &to_vec(&StatusDurations {
                    elapsed: Duration::from_millis(4444),
                    duration: Duration::from_millis(6666),
                })
                .expect("Serializer error")
            )
            .expect("Not utf8"),
            "time: 4:7\nelapsed: 4.444\nduration: 6.666"
        );
    }
}

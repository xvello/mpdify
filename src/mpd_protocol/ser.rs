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
    use crate::mpd_protocol::PlaybackStatus;
    use serde::Serialize;

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
}

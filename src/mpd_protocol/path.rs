use crate::mpd_protocol::InputError;
use crate::mpd_protocol::ItemType::{Album, Episode, Show, Track};
use crate::mpd_protocol::Path::{Empty, Internal};
use serde::{Serialize, Serializer};
use std::convert::AsRef;
use std::str::FromStr;
use strum::{AsRefStr, EnumString};

const SEPARATOR: char = '/';
const INTERNAL_PREFIX: &str = "internal";

#[derive(Debug, Eq, PartialEq, EnumString, AsRefStr, Clone)]
#[strum(serialize_all = "lowercase")]
pub enum ItemType {
    Track,
    Album,
    Show,
    Episode,
    Artist,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Path {
    Empty,
    Internal(Vec<(ItemType, String)>),
}

impl FromStr for Path {
    type Err = InputError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut tokens = s.split(SEPARATOR);
        match tokens.next() {
            None | Some("") => Ok(Empty),
            Some(INTERNAL_PREFIX) => {
                let mut items = vec![];
                while let Some(Ok(item_type)) = tokens.next().map(ItemType::from_str) {
                    match tokens.next() {
                        None | Some("") => {}
                        Some(id) => items.push((item_type, id.to_string())),
                    }
                }
                Ok(Internal(items))
            }
            Some(other) => Err(InputError::InvalidArgument("path", other.to_string())),
        }
    }
}

impl Serialize for Path {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}

impl ToString for Path {
    fn to_string(&self) -> String {
        match self {
            Empty => "".to_string(),
            Internal(items) => {
                let mut output = INTERNAL_PREFIX.to_string();
                for (item_type, id) in items {
                    output.push(SEPARATOR);
                    output.push_str(item_type.as_ref());
                    output.push(SEPARATOR);
                    output.push_str(id.as_str());
                }
                output
            }
        }
    }
}

impl Path {
    pub fn for_track(album_id: &str, track_id: &str) -> Self {
        Path::Internal(vec![
            (Album, album_id.to_string()),
            (Track, track_id.to_string()),
        ])
    }

    pub fn for_episode(show_id: &str, episode_id: &str) -> Self {
        Path::Internal(vec![
            (Show, show_id.to_string()),
            (Episode, episode_id.to_string()),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mpd_protocol::ItemType::{Album, Track};

    #[test]
    fn test_marshal_unmarshall() {
        let cases = vec![
            ("", Empty),
            (
                "internal/album/4IOXEu8EgItKI8J9JDaEr4/track/5fQP3T652SI6zdDaEtgwOd",
                Internal(vec![
                    (Album, "4IOXEu8EgItKI8J9JDaEr4".to_string()),
                    (Track, "5fQP3T652SI6zdDaEtgwOd".to_string()),
                ]),
            ),
            (
                "internal/album/4IOXEu8EgItKI8J9JDaEr4",
                Internal(vec![(Album, "4IOXEu8EgItKI8J9JDaEr4".to_string())]),
            ),
            (
                "internal/album/4IOXEu8EgItKI8J9JDaEr4/track/5fQP3T652SI6zdDaEtgwOd",
                Path::for_track("4IOXEu8EgItKI8J9JDaEr4", "5fQP3T652SI6zdDaEtgwOd"),
            ),
            (
                "internal/show/4IOXEu8EgItKI8J9JDaEr4/episode/5fQP3T652SI6zdDaEtgwOd",
                Path::for_episode("4IOXEu8EgItKI8J9JDaEr4", "5fQP3T652SI6zdDaEtgwOd"),
            ),
        ];

        for (text, variant) in cases {
            assert_eq!(text, &variant.to_string());
            assert_eq!(variant, Path::from_str(text).expect("Parsing error"));
        }
    }

    #[test]
    fn test_unmarshall_edge_cases() {
        let cases = vec![
            // Missing track ID (artwork for parent folder)
            (
                "internal/album/4IOXEu8EgItKI8J9JDaEr4/track/",
                Internal(vec![(Album, "4IOXEu8EgItKI8J9JDaEr4".to_string())]),
            ),
        ];

        for (text, variant) in cases {
            assert_eq!(variant, Path::from_str(text).expect("Parsing error"));
        }
    }
}

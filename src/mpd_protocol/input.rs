use enumset::EnumSetType;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

use crate::mpd_protocol::input::RelativeFloat::{Absolute, Relative};
use crate::mpd_protocol::Command;

/// Errors caused by invalid client input
#[derive(Error, Debug, PartialEq)]
pub enum InputError {
    #[error("invalid syntax '{0}'")]
    InvalidSyntax(String),
    #[error("no command")]
    MissingCommand,
    #[error("unknown command {0}")]
    UnknownCommand(String),
    #[error("missing argument {0}")]
    MissingArgument(String),
    #[error("invalid value for argument {0}: {1}")]
    InvalidArgument(String, String),
    #[error("cannot nest command lists")]
    NestedLists,
}

/// Supported subsystems for the idle command
/// See https://www.musicpd.org/doc/html/protocol.html#querying-mpd-s-status
#[derive(EnumSetType, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IdleSubsystem {
    #[serde(rename = "playlist")]
    PlayQueue,
    #[serde(rename = "stored_playlist")]
    Playlists,
    Player,
    Mixer,
    Options,
}

/// Parses a float, optionally prefixed by + or -
#[derive(Debug, Clone, PartialEq)]
pub enum RelativeFloat {
    Absolute(f64),
    Relative(f64),
}

impl FromStr for RelativeFloat {
    type Err = std::num::ParseFloatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match f64::from_str(s) {
            Ok(value) => {
                if s.starts_with('+') || s.starts_with('-') {
                    Ok(Relative(value))
                } else {
                    Ok(Absolute(value))
                }
            }
            Err(e) => Err(e),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PositionRange {
    pub start: usize,
    pub end: usize,
}

impl PositionRange {
    pub fn one(pos: usize) -> PositionRange {
        PositionRange {
            start: pos,
            end: pos + 1,
        }
    }

    pub fn contains(&self, pos: usize) -> bool {
        pos >= self.start && pos < self.end
    }
}

#[derive(Error, Debug, PartialEq)]
pub enum PositionRangeParsingErr {
    #[error(transparent)]
    ParseIntError(std::num::ParseIntError),
    #[error("invalid range format")]
    InvalidRange,
    #[error("end must be greater than start")]
    EmptyRange,
}

impl FromStr for PositionRange {
    type Err = PositionRangeParsingErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        match parts.len() {
            1 => {
                let pos =
                    usize::from_str(parts[0]).map_err(PositionRangeParsingErr::ParseIntError)?;
                Ok(PositionRange {
                    start: pos,
                    end: pos + 1,
                })
            }
            2 => {
                let start =
                    usize::from_str(parts[0]).map_err(PositionRangeParsingErr::ParseIntError)?;
                let end =
                    usize::from_str(parts[1]).map_err(PositionRangeParsingErr::ParseIntError)?;
                if end > start {
                    Ok(PositionRange { start, end })
                } else {
                    Err(PositionRangeParsingErr::EmptyRange)
                }
            }
            _ => Err(PositionRangeParsingErr::InvalidRange),
        }
    }
}

/// Holds several commands to be executed together
#[derive(Debug, PartialEq, Clone)]
pub struct CommandList {
    commands: Vec<Command>,
    verbose: bool,
}

impl CommandList {
    pub fn start(verbose: bool) -> Command {
        Command::CommandListStart(Self {
            commands: vec![],
            verbose,
        })
    }

    pub fn build(verbose: bool, commands: Vec<Command>) -> Command {
        Command::CommandListStart(Self { commands, verbose })
    }

    pub fn push(&mut self, command: Command) {
        self.commands.push(command)
    }

    pub fn get_commands(&self) -> Vec<Command> {
        self.commands.clone()
    }

    pub fn is_verbose(&self) -> bool {
        self.verbose
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_time_absolute() {
        assert_eq!(RelativeFloat::from_str("3.15").unwrap(), Absolute(3.15));
        assert_eq!(RelativeFloat::from_str("0.5").unwrap(), Absolute(0.5));
        assert_eq!(RelativeFloat::from_str("0").unwrap(), Absolute(0 as f64));
    }

    #[test]
    fn test_parse_time_relative() {
        assert_eq!(RelativeFloat::from_str("+3.15").unwrap(), Relative(3.15));
        assert_eq!(RelativeFloat::from_str("-9.99").unwrap(), Relative(-9.99));
        assert_eq!(RelativeFloat::from_str("-0").unwrap(), Relative(0 as f64));
    }

    #[test]
    fn test_parse_time_errors() {
        // TODO: can we assert on the error kind instead?
        assert!(RelativeFloat::from_str("")
            .err()
            .unwrap()
            .to_string()
            .contains("empty"));
        assert!(RelativeFloat::from_str("A")
            .err()
            .unwrap()
            .to_string()
            .contains("invalid"));
    }

    #[test]
    fn test_parse_position_range() {
        assert_eq!(
            PositionRange::from_str("18").unwrap(),
            PositionRange { start: 18, end: 19 }
        );
        assert_eq!(
            PositionRange::from_str("18:25").unwrap(),
            PositionRange { start: 18, end: 25 }
        );
    }

    #[test]
    fn test_parse_position_range_errors() {
        // TODO: can we assert on the error kind instead?
        assert!(PositionRange::from_str("")
            .err()
            .unwrap()
            .to_string()
            .contains("empty string"));
        assert!(PositionRange::from_str("A:B")
            .err()
            .unwrap()
            .to_string()
            .contains("invalid digit"));
        assert_eq!(
            PositionRange::from_str("1:2:3").err().unwrap(),
            PositionRangeParsingErr::InvalidRange
        );
        assert_eq!(
            PositionRange::from_str("1:1").err().unwrap(),
            PositionRangeParsingErr::EmptyRange
        );
    }

    #[test]
    fn test_command_list() {
        let command = CommandList::start(true);
        match command {
            Command::CommandListStart(mut list) => {
                assert_eq!(list.verbose, true);
                list.push(Command::Ping);
                list.push(Command::Pause(None));

                assert_eq!(
                    vec![Command::Ping, Command::Pause(None)],
                    list.get_commands()
                );
                assert!(list.is_verbose());
            }
            _ => panic!("Should be a command list start command"),
        }
    }
}

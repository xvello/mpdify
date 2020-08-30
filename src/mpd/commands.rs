use crate::mpd::types::{Time, MpdError};
use std::str::FromStr;
use crate::mpd::types::MpdError::{UnknownCommand, MissingArgument, InvalidArgument, MissingCommand};
use crate::mpd::commands::Command::{Pause, SeekCur};

// From https://www.musicpd.org/doc/html/protcurrentsongocol.html
#[derive (Debug, PartialEq)]
pub enum Command {
    // Status commands
    ClearError,
    CurrentSong,
    Idle,
    Status,
    Stats,

    // Playback options

    // Playback control
    Next,
    Pause(bool),
    Play(Option<u32>),
    PlayId(Option<u32>),
    Previous,
    // Seek, SeekId
    SeekCur(Time),
    Stop,

    // Connection settings
    Ping,
}

impl FromStr for Command {
    type Err = MpdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // TODO: need to support quoting arguments (custom iterator)
        let mut tokens = s.split_ascii_whitespace();

        tokens.next()
            .ok_or(MissingCommand)
            .and_then(|command| match command {
                // Status commands
                "clearerror" => Ok(Command::ClearError),
                "currentsong" => Ok(Command::CurrentSong),
                "idle" => Ok(Command::Idle),
                "status" => Ok(Command::Status),
                "stats" => Ok(Command::Stats),

                // Playback control
                "next" => Ok(Command::Next),
                "pause" => {
                    parse_argument("paused".to_string(), tokens.next())
                        .map(|v: i8| v > 0)
                        .map(Pause)
                },
                "previous" => Ok(Command::Previous),
                "seekcur" => {
                    parse_argument("time".to_string(), tokens.next())
                        .map(SeekCur)
                }
                "stop" => Ok(Command::Stop),

                // Connection settings
                "ping" => Ok(Command::Ping),

                // Unknown command
                _ => Err(UnknownCommand(command.to_string()))
            })
    }
}

fn parse_argument<T: FromStr>(name: String, token: Option<&str>) -> Result<T, MpdError> {
    token.ok_or(MissingArgument(name.clone()))
        .and_then(|v| T::from_str(v)
            // TODO: propagate parsing error
            .map_err(|_e| InvalidArgument(name)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mpd::commands::Command::Ping;
    use crate::mpd::types::Time::{AbsoluteSeconds, RelativeSeconds};

    #[test]
    fn test_no_command() {
        assert_eq!(Command::from_str("").err().unwrap(), MissingCommand);
    }

    #[test]
    fn test_unknown_command() {
        assert_eq!(Command::from_str("unknown").err().unwrap(), UnknownCommand("unknown".to_string()));
    }

    #[test]
    fn test_ping() {
        assert_eq!(Command::from_str("ping").unwrap(), Ping);
    }

    #[test]
    fn test_pause() {
        assert_eq!(Command::from_str("pause 1").unwrap(), Pause(true));
        assert_eq!(Command::from_str("pause 0").unwrap(), Pause(false));

        assert_eq!(Command::from_str("pause").err().unwrap(), MissingArgument("paused".to_string()));
        assert_eq!(Command::from_str("pause A").err().unwrap(), InvalidArgument("paused".to_string()));
    }

    #[test]
    fn test_seek_cur() {
        assert_eq!(Command::from_str("seekcur 23.3").unwrap(), SeekCur(AbsoluteSeconds(23.3)));
        assert_eq!(Command::from_str("seekcur +0.3").unwrap(), SeekCur(RelativeSeconds(0.3)));
        assert_eq!(Command::from_str("seekcur -2").unwrap(), SeekCur(RelativeSeconds(-2.0)));

        assert_eq!(Command::from_str("seekcur").err().unwrap(), MissingArgument("time".to_string()));
        assert_eq!(Command::from_str("seekcur A").err().unwrap(), InvalidArgument("time".to_string()));
    }
}

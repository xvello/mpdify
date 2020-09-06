use std::str::FromStr;

use crate::mpd::commands::Command::{Pause, SeekCur};
use crate::mpd::inputtypes::InputError::{
    InvalidArgument, MissingArgument, MissingCommand, UnknownCommand,
};
use crate::mpd::inputtypes::{InputError, Time};
use std::borrow::Borrow;

// From https://www.musicpd.org/doc/html/protcurrentsongocol.html
#[derive(Debug, PartialEq, Clone)]
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
    Close,
}

impl FromStr for Command {
    type Err = InputError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let tokenized = tokenize_command(s);
        let mut tokens = tokenized.iter();

        tokens
            .next()
            .ok_or(MissingCommand)
            .and_then(|command| match command.borrow() {
                // Status commands
                "clearerror" => Ok(Command::ClearError),
                "currentsong" => Ok(Command::CurrentSong),
                "idle" => Ok(Command::Idle),
                "status" => Ok(Command::Status),
                "stats" => Ok(Command::Stats),

                // Playback control
                "next" => Ok(Command::Next),
                "pause" => parse_argument("paused".to_string(), tokens.next())
                    .map(|v: i8| v > 0)
                    .map(Pause),
                "previous" => Ok(Command::Previous),
                "seekcur" => parse_argument("time".to_string(), tokens.next()).map(SeekCur),
                "stop" => Ok(Command::Stop),

                // Connection settings
                "ping" => Ok(Command::Ping),
                "close" => Ok(Command::Close),

                // Unknown command
                _ => Err(UnknownCommand(command.to_string())),
            })
    }
}

fn parse_argument<T: FromStr>(name: String, token: Option<&String>) -> Result<T, InputError> {
    token
        .ok_or_else(|| MissingArgument(name.clone()))
        .and_then(|v| {
            T::from_str(v.borrow())
                // TODO: propagate parsing error
                .map_err(|_e| InvalidArgument(name, v.to_string()))
        })
}

fn tokenize_command(input: &str) -> Vec<String> {
    let mut tokens = vec![];
    let mut is_escaped = false;
    let mut is_quoted = false;
    let mut current = String::new();

    for char in input.chars() {
        // Copy escaped special characters
        if is_escaped {
            current.push(char);
            is_escaped = false;
            continue;
        }

        match char {
            '\\' => is_escaped = true,
            '"' | '\'' => is_quoted = !is_quoted,
            w if w.is_whitespace() => {
                if is_quoted {
                    current.push(char)
                } else if !current.is_empty() {
                    tokens.push(current);
                    current = String::new();
                }
            }
            _ => current.push(char),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mpd::commands::Command::Ping;
    use crate::mpd::inputtypes::Time::{AbsoluteSeconds, RelativeSeconds};

    #[test]
    fn test_no_command() {
        assert_eq!(Command::from_str("").err().unwrap(), MissingCommand);
    }

    #[test]
    fn test_unknown_command() {
        assert_eq!(
            Command::from_str("unknown").err().unwrap(),
            UnknownCommand("unknown".to_string())
        );
    }

    #[test]
    fn test_ping() {
        assert_eq!(Command::from_str("ping").unwrap(), Ping);
    }

    #[test]
    fn test_pause() {
        assert_eq!(Command::from_str("pause 1").unwrap(), Pause(true));
        assert_eq!(Command::from_str("pause 0").unwrap(), Pause(false));

        assert_eq!(
            Command::from_str("pause").err().unwrap(),
            MissingArgument("paused".to_string())
        );
        assert_eq!(
            Command::from_str("pause A").err().unwrap(),
            InvalidArgument("paused".to_string(), "A".to_string())
        );
    }

    #[test]
    fn test_seek_cur() {
        assert_eq!(
            Command::from_str("seekcur 23.3").unwrap(),
            SeekCur(AbsoluteSeconds(23.3))
        );
        assert_eq!(
            Command::from_str("seekcur +0.3").unwrap(),
            SeekCur(RelativeSeconds(0.3))
        );
        assert_eq!(
            Command::from_str("seekcur -2").unwrap(),
            SeekCur(RelativeSeconds(-2.0))
        );

        assert_eq!(
            Command::from_str("seekcur").err().unwrap(),
            MissingArgument("time".to_string())
        );
        assert_eq!(
            Command::from_str("seekcur A").err().unwrap(),
            InvalidArgument("time".to_string(), "A".to_string())
        );
    }

    #[test]
    fn test_tokenize_command() {
        assert_eq!(tokenize_command("test"), vec!["test"]);
        assert_eq!(tokenize_command("simple space"), vec!["simple", "space"]);
        assert_eq!(tokenize_command("\"double quoted\""), vec!["double quoted"]);
        assert_eq!(tokenize_command("\'single quoted\'"), vec!["single quoted"]);
        assert_eq!(tokenize_command("\'quo\\\' ted\'"), vec!["quo\' ted"]);
        assert_eq!(tokenize_command("empty last \"\""), vec!["empty", "last"]);
        assert_eq!(
            tokenize_command("command \"quoted arg\" unquoted\\\' arg "),
            vec!["command", "quoted arg", "unquoted\'", "arg"]
        );

        assert_eq!(tokenize_command("slashed\\\\"), vec!["slashed\\"]);
        assert_eq!(tokenize_command("pause 1"), vec!["pause", "1"]);
        assert_eq!(tokenize_command("pause \"1\""), vec!["pause", "1"]);
    }
}

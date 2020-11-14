use std::str::FromStr;

use crate::mpd_protocol::commands::Command::{
    ChangeVolume, Pause, Play, PlayId, SeekCur, SetVolume, SpotifyAuth,
};
use crate::mpd_protocol::input::InputError::{
    InvalidArgument, MissingArgument, MissingCommand, UnknownCommand,
};
use crate::mpd_protocol::input::{InputError, RelativeFloat};
use crate::mpd_protocol::Command::{PlaylistId, PlaylistInfo};
use crate::mpd_protocol::{CommandList, PositionRange};
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

    // Playlist info
    PlaylistInfo(Option<PositionRange>), // End is exclusive
    PlaylistId(Option<usize>),

    // Playback options

    // Playback control
    Next,
    Pause(Option<bool>), // None means toggle
    Play(Option<u32>),   // None means unpause
    PlayId(Option<u32>),
    Previous,
    // Seek, SeekId
    SeekCur(RelativeFloat), // Seconds
    Stop,

    // Volume
    SetVolume(u32),    // Absolute value
    ChangeVolume(i32), // Relative change

    // Connection settings
    Ping,
    Close,

    // Command list
    CommandListStart(CommandList),
    CommandListEnd,

    // Custom extension to support oauth2 authentication
    SpotifyAuth(Option<String>),
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

                // Playlist info
                "playlistinfo" => parse_opt("range".to_string(), tokens.next()).map(PlaylistInfo),
                "playlistid" => parse_opt("songid".to_string(), tokens.next()).map(PlaylistId),

                // Playback control
                "next" => Ok(Command::Next),
                "pause" => parse_opt("paused".to_string(), tokens.next())
                    .map(|o: Option<i8>| o.map(|v| v > 0))
                    .map(Pause),
                "previous" => Ok(Command::Previous),
                "seekcur" => parse_arg("time".to_string(), tokens.next()).map(SeekCur),
                "stop" => Ok(Command::Stop),
                "play" => parse_opt("pos".to_string(), tokens.next()).map(Play),
                "playid" => parse_opt("id".to_string(), tokens.next()).map(PlayId),

                // Volume
                "setvol" => parse_arg("vol".to_string(), tokens.next()).map(SetVolume),
                "volume" => parse_arg("change".to_string(), tokens.next()).map(ChangeVolume),

                // Connection settings
                "ping" => Ok(Command::Ping),
                "close" => Ok(Command::Close),

                // Command list
                "command_list_begin" => Ok(CommandList::start(false)),
                "command_list_ok_begin" => Ok(CommandList::start(true)),
                "command_list_end" => Ok(Command::CommandListEnd),

                // Custom extension to support oauth2 authentication
                "auth" => parse_opt("url".to_string(), tokens.next()).map(SpotifyAuth),

                // Unknown command
                _ => Err(UnknownCommand(command.to_string())),
            })
    }
}

fn parse_arg<T: FromStr>(name: String, token: Option<&String>) -> Result<T, InputError> {
    let parsed: Option<T> = parse_opt(name.clone(), token)?;
    match parsed {
        None => Err(MissingArgument(name)),
        Some(v) => Ok(v),
    }
}

fn parse_opt<T: FromStr>(name: String, token: Option<&String>) -> Result<Option<T>, InputError> {
    match token {
        None => Ok(None),
        Some(v) => {
            T::from_str(v.borrow())
                .map(Option::Some)
                // TODO: propagate parsing error
                .map_err(|_e| InvalidArgument(name, v.to_string()))
        }
    }
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
    use crate::mpd_protocol::commands::Command::Ping;
    use crate::mpd_protocol::input::RelativeFloat::{Absolute, Relative};

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
        assert_eq!(Command::from_str("pause 1").unwrap(), Pause(Some(true)));
        assert_eq!(Command::from_str("pause 0").unwrap(), Pause(Some(false)));
        assert_eq!(Command::from_str("pause").unwrap(), Pause(None));

        assert_eq!(
            Command::from_str("pause A").err().unwrap(),
            InvalidArgument("paused".to_string(), "A".to_string())
        );
    }

    #[test]
    fn test_seek_cur() {
        assert_eq!(
            Command::from_str("seekcur 23.3").unwrap(),
            SeekCur(Absolute(23.3))
        );
        assert_eq!(
            Command::from_str("seekcur +0.3").unwrap(),
            SeekCur(Relative(0.3))
        );
        assert_eq!(
            Command::from_str("seekcur -2").unwrap(),
            SeekCur(Relative(-2.0))
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

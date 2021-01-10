use crate::mpd_protocol::commands::Command::{
    ChangeVolume, Pause, PlayId, PlayPos, SeekCur, SetVolume, SpotifyAuth,
};
use crate::mpd_protocol::input::InputError::{
    InvalidArgument, MissingArgument, MissingCommand, UnknownCommand,
};
use crate::mpd_protocol::input::{InputError, RelativeFloat};
use crate::mpd_protocol::Command::{PlaylistId, PlaylistInfo};
use crate::mpd_protocol::{CommandList, IdleSubsystem, PositionRange};
use enumset::EnumSet;
use log::debug;
use std::borrow::Borrow;
use std::str::FromStr;

// From https://www.musicpd.org/doc/html/protcurrentsongocol.html
#[derive(Debug, PartialEq, Clone)]
pub enum Command {
    // Status commands
    ClearError,
    CurrentSong,
    Idle(EnumSet<IdleSubsystem>),
    NoIdle,
    Status,
    Stats,

    // Playlist info
    PlaylistInfo(Option<PositionRange>), // End is exclusive
    PlaylistId(Option<usize>),

    // Playback options

    // Playback control
    Next,
    Pause(Option<bool>),    // None means toggle
    PlayPos(Option<usize>), // None means unpause, position >=0
    PlayId(Option<usize>),  // None means unpause, id > 0
    Previous,
    // Seek, SeekId
    SeekCur(RelativeFloat), // Seconds
    Stop,

    // Volume
    GetVolume,
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
        Command::from_tokens(tokenized.iter().map(String::as_str))
    }
}

impl Command {
    pub fn from_tokens<'a, I>(mut tokens: I) -> Result<Self, InputError>
    where
        I: Iterator<Item = &'a str>,
    {
        tokens
            .next()
            .ok_or(MissingCommand)
            .and_then(|command| match command.borrow() {
                // Status commands
                "clearerror" => Ok(Command::ClearError),
                "currentsong" => Ok(Command::CurrentSong),
                "status" => Ok(Command::Status),
                "stats" => Ok(Command::Stats),

                // Idle
                "idle" => {
                    let mut subsytems: EnumSet<IdleSubsystem> = EnumSet::empty();
                    for name in tokens {
                        match serde_yaml::from_str(name) {
                            Ok(subsytem) => {
                                subsytems.insert(subsytem);
                            }
                            Err(_) => debug!["Ignoring unsupported idle subsystem {}", name],
                        }
                    }
                    if subsytems.is_empty() {
                        subsytems = EnumSet::all();
                    }
                    Ok(Command::Idle(subsytems))
                }
                "noidle" => Ok(Command::NoIdle),

                // Playlist info
                "playlistinfo" => parse_opt("range".to_string(), tokens.next()).map(PlaylistInfo),
                "playlistid" => parse_opt("songid".to_string(), tokens.next())
                    .and_then(check_song_id)
                    .map(PlaylistId),

                // Playback control
                "next" => Ok(Command::Next),
                "pause" => parse_opt("paused".to_string(), tokens.next())
                    .map(|o: Option<i8>| o.map(|v| v > 0))
                    .map(Pause),
                "previous" => Ok(Command::Previous),
                "seekcur" => parse_arg("time".to_string(), tokens.next()).map(SeekCur),
                "stop" => Ok(Command::Stop),
                "play" => parse_opt("pos".to_string(), tokens.next()).map(PlayPos),
                "playid" => parse_opt("songid".to_string(), tokens.next())
                    .and_then(check_song_id)
                    .map(PlayId),

                // Volume
                "getvol" => Ok(Command::GetVolume),
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

                // Unsupported commands we just map to a ping
                "channels" | "subscribe" | "unsubscribe" | "readmessages" | "sendmessage"
                | "consume" | "crossfade" | "mixrampdb" | "mixrampdelay" | "replay_gain_mode"
                | "replay_gain_status" | "outputs" => Ok(Command::Ping),

                // Unknown command
                _ => Err(UnknownCommand(command.to_string())),
            })
    }
}

fn parse_arg<T: FromStr>(name: String, token: Option<&str>) -> Result<T, InputError> {
    let parsed: Option<T> = parse_opt(name.clone(), token)?;
    match parsed {
        None => Err(MissingArgument(name)),
        Some(v) => Ok(v),
    }
}

fn parse_opt<T: FromStr>(name: String, token: Option<&str>) -> Result<Option<T>, InputError> {
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

/// Ensures song IDs are strictly higher than zero (invalid value)
fn check_song_id(id: Option<usize>) -> Result<Option<usize>, InputError> {
    match id {
        Some(0) => Err(InvalidArgument("songid".to_string(), "0".to_string())),
        _ => Ok(id),
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
    use crate::mpd_protocol::Command::Idle;

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
    fn test_playid() {
        assert_eq!(Command::from_str("playid 1").unwrap(), PlayId(Some(1)));
        assert_eq!(Command::from_str("playid").unwrap(), PlayId(None));

        assert_eq!(
            Command::from_str("playid A").err().unwrap(),
            InvalidArgument("songid".to_string(), "A".to_string())
        );
        assert_eq!(
            Command::from_str("playid 0").err().unwrap(),
            InvalidArgument("songid".to_string(), "0".to_string())
        );
        assert_eq!(
            Command::from_str("playid -10").err().unwrap(),
            InvalidArgument("songid".to_string(), "-10".to_string())
        );
    }

    #[test]
    fn test_idle() {
        assert_eq!(Command::from_str("idle").unwrap(), Idle(EnumSet::all()));
        assert_eq!(
            Command::from_str("idle playlist unknown mixer").unwrap(),
            Idle(IdleSubsystem::PlayQueue | IdleSubsystem::Mixer)
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

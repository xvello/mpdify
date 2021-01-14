use crate::mpd_protocol::commands::Command::{
    ChangeVolume, EnableOutput, Pause, PlayId, PlayPos, PlaylistId, PlaylistInfo, Random, Repeat,
    RepeatSingle, SeekCur, SeekId, SeekPos, SetVolume, SpotifyAuth,
};
use crate::mpd_protocol::input::InputError::{
    InvalidArgument, MissingArgument, MissingCommand, UnknownCommand,
};
use crate::mpd_protocol::input::{InputError, RelativeFloat};
use crate::mpd_protocol::{CommandList, IdleSubsystem, PositionRange};
use enumset::EnumSet;
use log::debug;
use std::str::FromStr;

// From https://www.musicpd.org/doc/html/protocol.html
#[derive(Debug, PartialEq, Clone)]
pub enum Command {
    // Status commands
    CurrentSong,
    Idle(EnumSet<IdleSubsystem>),
    NoIdle,
    Status,
    Stats,
    Commands,

    // Outputs
    Outputs,
    EnableOutput(usize),

    // Playlist info
    PlaylistInfo(Option<PositionRange>), // End is exclusive
    PlaylistId(Option<usize>),

    // Playback options
    Random(bool),
    Repeat(bool),
    RepeatSingle(bool),

    // Playback control
    Next,
    Pause(Option<bool>),    // None means toggle
    PlayPos(Option<usize>), // None means unpause, position >=0
    PlayId(Option<usize>),  // None means unpause, id > 0
    Previous,
    SeekId(usize, f64),
    SeekPos(usize, f64),
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
        Command::from_tokens(tokenize_command(s))
    }
}

impl Command {
    pub fn known_commands() -> Vec<&'static str> {
        vec![
            "currentsong",
            "status",
            "commands",
            "idle",
            "noidle",
            "playlistinfo",
            "playlistid",
            "random",
            "repeat",
            "single",
            "next",
            "pause",
            "previous",
            "seekcur",
            "seekid",
            "seekpos",
            "stop",
            "play",
            "playid",
            "getvol",
            "setvol",
            "volume",
            "ping",
            "close",
            "command_list_begin",
            "command_list_ok_begin",
            "command_list_end",
            "auth",
            "outputs",
            "toggleoutput",
            "enableoutput",
        ]
    }

    pub fn from_tokens(tokens: Vec<String>) -> Result<Self, InputError> {
        let mut args = Arguments::from_vec(tokens);
        args.command().and_then(|command| match command.as_ref() {
            // Status commands
            "currentsong" => Ok(Command::CurrentSong),
            "status" => Ok(Command::Status),
            "stats" => Ok(Command::Stats),
            "commands" => Ok(Command::Commands),

            // Outputs
            "outputs" => Ok(Command::Outputs),
            "toggleoutput" | "enableoutput" => args.req("id").map(EnableOutput),

            // Idle
            "idle" => {
                if args.is_empty() {
                    return Ok(Command::Idle(EnumSet::all()));
                }
                let mut subsytems: EnumSet<IdleSubsystem> = EnumSet::empty();
                while let Some(name) = args.pop() {
                    match serde_yaml::from_str(&name) {
                        Ok(subsytem) => {
                            subsytems.insert(subsytem);
                        }
                        Err(_) => debug!["Ignoring unsupported idle subsystem {}", name],
                    }
                }
                Ok(Command::Idle(subsytems))
            }
            "noidle" => Ok(Command::NoIdle),

            // Playlist info
            "playlistinfo" => args.opt("range").map(PlaylistInfo),
            "playlistid" => args.opt("songid").and_then(check_song_id).map(PlaylistId),

            // Playback options
            "random" => args.req("state").map(int_to_bool).map(Random),
            "repeat" => args.req("state").map(int_to_bool).map(Repeat),
            "single" => args.req("state").map(int_to_bool).map(RepeatSingle),

            // Playback control
            "next" => Ok(Command::Next),
            "pause" => args.opt("paused").map(|v| v.map(int_to_bool)).map(Pause),
            "previous" => Ok(Command::Previous),
            "seekcur" => args.req("time").map(SeekCur),
            "seekid" => Ok(SeekId(args.req("songid")?, args.req("time")?)),
            "seekpos" => Ok(SeekPos(args.req("songpos")?, args.req("time")?)),
            "stop" => Ok(Command::Stop),
            "play" => args.opt("pos").map(PlayPos),
            "playid" => args.opt("songid").and_then(check_song_id).map(PlayId),

            // Volume
            "getvol" => Ok(Command::GetVolume),
            "setvol" => args.req("vol").map(SetVolume),
            "volume" => args.req("change").map(ChangeVolume),

            // Connection settings
            "ping" => Ok(Command::Ping),
            "close" => Ok(Command::Close),

            // Command list
            "command_list_begin" => Ok(CommandList::start(false)),
            "command_list_ok_begin" => Ok(CommandList::start(true)),
            "command_list_end" => Ok(Command::CommandListEnd),

            // Custom extension to support oauth2 authentication
            "auth" => args.opt("url").map(SpotifyAuth),

            // Unsupported commands we just map to a ping
            "clearerror" | "channels" | "subscribe" | "unsubscribe" | "readmessages"
            | "sendmessage" | "consume" | "crossfade" | "mixrampdb" | "mixrampdelay"
            | "replay_gain_mode" | "replay_gain_status" | "disableoutput" => Ok(Command::Ping),

            // Unknown command
            _ => Err(UnknownCommand(command)),
        })
    }
}

struct Arguments(Vec<String>);

impl Arguments {
    pub fn from_vec(mut tokens: Vec<String>) -> Self {
        tokens.reverse();
        Self(tokens)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn pop(&mut self) -> Option<String> {
        self.0.pop()
    }

    pub fn command(&mut self) -> Result<String, InputError> {
        match self.0.pop() {
            None => Err(MissingCommand),
            Some(v) => Ok(v),
        }
    }

    pub fn req<T: FromStr>(&mut self, name: &'static str) -> Result<T, InputError> {
        match self.opt(name)? {
            Some(v) => Ok(v),
            None => Err(MissingArgument(name)),
        }
    }

    fn opt<T: FromStr>(&mut self, name: &'static str) -> Result<Option<T>, InputError> {
        match self.0.pop() {
            None => Ok(None),
            Some(v) => T::from_str(&v)
                .map(Option::Some)
                .map_err(|_e| InvalidArgument(name, v.to_string())),
        }
    }
}

fn int_to_bool(value: u8) -> bool {
    value > 0
}

/// Ensures song IDs are strictly higher than zero (invalid value)
fn check_song_id(id: Option<usize>) -> Result<Option<usize>, InputError> {
    match id {
        Some(0) => Err(InvalidArgument("songid", "0".to_string())),
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
            InvalidArgument("paused", "A".to_string())
        );
    }

    #[test]
    fn test_playid() {
        assert_eq!(Command::from_str("playid 1").unwrap(), PlayId(Some(1)));
        assert_eq!(Command::from_str("playid").unwrap(), PlayId(None));

        assert_eq!(
            Command::from_str("playid A").err().unwrap(),
            InvalidArgument("songid", "A".to_string())
        );
        assert_eq!(
            Command::from_str("playid 0").err().unwrap(),
            InvalidArgument("songid", "0".to_string())
        );
        assert_eq!(
            Command::from_str("playid -10").err().unwrap(),
            InvalidArgument("songid", "-10".to_string())
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
            MissingArgument("time")
        );
        assert_eq!(
            Command::from_str("seekcur A").err().unwrap(),
            InvalidArgument("time", "A".to_string())
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

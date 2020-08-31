use std::str::FromStr;
use thiserror::Error;

use crate::mpd::inputtypes::Time::{RelativeSeconds, AbsoluteSeconds};

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
    #[error("invalid value for argument {0}")]
    InvalidArgument(String),
}

/// Parses a float, optionally prefixed by + or -
#[derive (Debug, PartialEq)]
pub enum Time {
    AbsoluteSeconds(f64),
    RelativeSeconds(f64),
}

impl FromStr for Time {
    type Err = std::num::ParseFloatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match f64::from_str(s) {
            Ok(value) => {
                if s.starts_with('+') || s.starts_with('-') {
                    Ok(RelativeSeconds(value))
                } else {
                    Ok(AbsoluteSeconds(value))
                }
            },
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_time_absolute() {
        assert_eq!(Time::from_str("3.14").unwrap(), AbsoluteSeconds(3.14));
        assert_eq!(Time::from_str("0.5").unwrap(), AbsoluteSeconds(0.5));
        assert_eq!(Time::from_str("0").unwrap(), AbsoluteSeconds(0 as f64));
    }

    #[test]
    fn test_parse_time_relative() {
        assert_eq!(Time::from_str("+3.14").unwrap(), RelativeSeconds(3.14));
        assert_eq!(Time::from_str("-9.99").unwrap(), RelativeSeconds(-9.99));
        assert_eq!(Time::from_str("-0").unwrap(), RelativeSeconds(0 as f64));
    }

    #[test]
    fn test_parse_time_errors() {
        // TODO: can we assert on the error kind instead?
        assert!(Time::from_str("").err().unwrap().to_string().contains("empty"));
        assert!(Time::from_str("A").err().unwrap().to_string().contains("invalid"));
    }
}

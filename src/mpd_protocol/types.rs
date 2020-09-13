use std::str::FromStr;
use thiserror::Error;

use crate::mpd_protocol::types::RelativeFloat::{Absolute, Relative};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_time_absolute() {
        assert_eq!(RelativeFloat::from_str("3.14").unwrap(), Absolute(3.14));
        assert_eq!(RelativeFloat::from_str("0.5").unwrap(), Absolute(0.5));
        assert_eq!(RelativeFloat::from_str("0").unwrap(), Absolute(0 as f64));
    }

    #[test]
    fn test_parse_time_relative() {
        assert_eq!(RelativeFloat::from_str("+3.14").unwrap(), Relative(3.14));
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
}

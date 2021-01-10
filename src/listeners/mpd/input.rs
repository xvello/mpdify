use crate::listeners::mpd::types::ListenerError;
use crate::mpd_protocol::Command::CommandListStart;
use crate::mpd_protocol::*;
use log::debug;
use std::borrow::Borrow;
use std::str::FromStr;
use tokio_stream::{Stream, StreamExt};

/// Reads the next command from the client
pub async fn read_command<T>(lines: &mut T) -> Result<Command, ListenerError>
where
    T: Stream<Item = std::io::Result<String>> + Unpin,
{
    let command = read_one_command(lines).await?;

    match command {
        Command::CommandListEnd => Err(ListenerError::InputError(InputError::MissingCommand)),
        Command::CommandListStart(mut list) => loop {
            let nested = read_one_command(lines).await?;
            match nested {
                Command::CommandListStart(_) => {
                    return Err(ListenerError::InputError(InputError::NestedLists));
                }
                Command::CommandListEnd => return Ok(CommandListStart(list)),
                _ => list.push(nested),
            }
        },
        _ => Ok(command),
    }
}

async fn read_one_command<T>(lines: &mut T) -> Result<Command, ListenerError>
where
    T: Stream<Item = std::io::Result<String>> + Unpin,
{
    let line = lines.next().await;
    match line {
        None => Err(ListenerError::ConnectionClosed),
        Some(line) => match line {
            Err(err) => Err(ListenerError::IO(err)),
            Ok(line) => {
                debug!("Read command {:?}", line);
                Command::from_str(line.borrow()).map_err(ListenerError::InputError)
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Result;
    use tokio::stream;
    use tokio_stream::Stream;

    struct Lines {
        items: Box<dyn Stream<Item = std::io::Result<String>> + Unpin>,
    }

    impl Lines {
        pub fn from_str(lines: Vec<&str>) -> Self {
            let mut results: Vec<Result<String>> = vec![];
            for line in lines {
                results.push(Ok(line.to_string()));
            }
            let items = Box::new(stream::iter(results));
            Lines { items }
        }

        pub async fn assert_command(&mut self, expected: Command) {
            assert_eq!(
                read_command(&mut self.items)
                    .await
                    .expect("Unexpected error"),
                expected
            )
        }

        pub async fn assert_input_error(&mut self, expected: InputError) {
            match read_command(&mut self.items)
                .await
                .expect_err("Expected error")
            {
                ListenerError::InputError(err) => assert_eq!(err, expected),
                err => panic!["Expected {:?}, got {:?}", expected, err],
            }
        }

        pub async fn assert_closed(&mut self) {
            match read_command(&mut self.items)
                .await
                .expect_err("Expected error")
            {
                ListenerError::ConnectionClosed => {}
                err => panic!["Unexpected error {:?}", err],
            }
        }
    }

    #[tokio::test]
    async fn it_closes_on_empty_input() {
        let mut input = Lines::from_str(vec![]);
        input.assert_closed().await;
    }

    #[tokio::test]
    async fn it_parses_simple_commands() {
        let mut input = Lines::from_str(vec!["status", "ping"]);
        input.assert_command(Command::Status).await;
        input.assert_command(Command::Ping).await;
        input.assert_closed().await;
    }

    #[tokio::test]
    async fn it_propagates_parsing_errors() {
        let mut input = Lines::from_str(vec!["volume", "volume A", "unknown"]);
        input
            .assert_input_error(InputError::MissingArgument("change".to_string()))
            .await;
        input
            .assert_input_error(InputError::InvalidArgument(
                "change".to_string(),
                "A".to_string(),
            ))
            .await;
        input
            .assert_input_error(InputError::UnknownCommand("unknown".to_string()))
            .await;
        input.assert_closed().await;
    }

    #[tokio::test]
    async fn it_parses_command_lists() {
        let mut input = Lines::from_str(vec![
            "next",
            "command_list_ok_begin",
            "status",
            "ping",
            "command_list_end",
            "next",
        ]);
        input.assert_command(Command::Next).await;
        let expected_list = CommandList::build(true, vec![Command::Status, Command::Ping]);
        input.assert_command(expected_list).await;
        input.assert_command(Command::Next).await;
        input.assert_closed().await;
    }

    #[tokio::test]
    async fn it_rejects_nested_command_lists() {
        let mut input = Lines::from_str(vec![
            "command_list_ok_begin",
            "command_list_ok_begin",
            "next",
        ]);
        input.assert_input_error(InputError::NestedLists).await;
        input.assert_command(Command::Next).await;
        input.assert_closed().await;
    }

    #[tokio::test]
    async fn it_closes_on_incomplete_command_lists() {
        let mut input = Lines::from_str(vec!["command_list_ok_begin"]);
        input.assert_closed().await;
    }

    #[tokio::test]
    async fn it_rejects_command_list_end_before_begin() {
        let mut input = Lines::from_str(vec!["command_list_end"]);
        input.assert_input_error(InputError::MissingCommand).await;
        input.assert_closed().await;
    }
}

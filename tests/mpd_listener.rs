use log::{debug, warn};
use mpdify::listeners::mpd::MpdListener;
use mpdify::mpd_protocol::{Command, HandlerError, HandlerInput, HandlerOutput, PlaybackStatus};
use serde::Serialize;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::{Acquire, Release};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn it_handles_two_connections() {
    init_logger();

    let mut listener = MpdListener::new("127.0.0.1:0".to_string(), vec![]).await;
    let address = listener.get_address().expect("Cannot get server address");

    debug!("Listening on random port {}", address);
    tokio::spawn(async move { listener.run().await });

    let mut clients = vec![
        Client::new(address.clone()).await,
        Client::new(address.clone()).await,
    ];

    for client in clients.iter_mut().rev() {
        client.send_command("ping").await;
        client.assert_response("OK\n".to_string()).await;
    }

    for client in clients.iter_mut() {
        client.send_command("close").await;
        client.assert_no_response().await;
    }
}

#[tokio::test]
async fn it_calls_custom_handler() {
    init_logger();

    // Run custom handler
    let (mut handler, pause_tx, is_paused) = CustomHandler::new();
    tokio::spawn(async move { handler.run().await });

    // Run listener
    let mut listener = MpdListener::new("127.0.0.1:0".to_string(), vec![pause_tx]).await;
    let address = listener.get_address().expect("Cannot get server address");
    debug!("Listening on random port {}", address);
    tokio::spawn(async move { listener.run().await });

    let mut client = Client::new(address.clone()).await;

    // Pause command is sent to our handler
    client.send_command("pause 1").await;
    client.assert_response("OK\n".to_string()).await;
    assert!(is_paused.load(Acquire));
    client.send_command("pause \"0\"").await;
    client.assert_response("OK\n".to_string()).await;
    assert!(!is_paused.load(Acquire));

    // stats command is sent to our handler
    client.send_command("stats").await;
    client
        .assert_response("one: 1\ntwo: 2\nOK\n".to_string())
        .await;

    // status command is sent to our handler
    client.send_command("status").await;
    client
        .assert_response("volume: 20\nstate: pause\nOK\n".to_string())
        .await;

    // Ping is still handled by the default handler
    client.send_command("ping").await;
    client.assert_response("OK\n".to_string()).await;
    client.send_command("close").await;
    client.assert_no_response().await;
}

#[tokio::test]
async fn it_supports_command_lists() {
    init_logger();

    // Run custom handler
    let (mut handler, pause_tx, _) = CustomHandler::new();
    tokio::spawn(async move { handler.run().await });

    // Run listener
    let mut listener = MpdListener::new("127.0.0.1:0".to_string(), vec![pause_tx]).await;
    let address = listener.get_address().expect("Cannot get server address");
    debug!("Listening on random port {}", address);
    tokio::spawn(async move { listener.run().await });

    let mut client = Client::new(address.clone()).await;
    let stats = "one: 1\ntwo: 2\n";

    // We only get a single OK by default
    client.send_commands(vec!["stats", "stats"], false).await;
    client
        .assert_response(format!["{}{}OK\n", stats, stats])
        .await;

    // We expect list_OK between each answer
    client.send_commands(vec!["stats", "stats"], true).await;
    client
        .assert_response(format!["{}list_OK\n{}list_OK\nOK\n", stats, stats])
        .await;
}

fn init_logger() {
    let _ = pretty_env_logger::try_init();
}

#[derive(Debug, PartialEq, Serialize)]
pub struct CustomStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<u32>,
    pub state: PlaybackStatus,
}

struct CustomHandler {
    is_paused: Arc<AtomicBool>,
    rx: Receiver<HandlerInput>,
}

impl CustomHandler {
    fn new() -> (Self, Sender<HandlerInput>, Arc<AtomicBool>) {
        let (tx, rx) = mpsc::channel(16);
        let is_paused = Arc::new(AtomicBool::new(false));
        let cloned_paused = is_paused.clone();
        (Self { is_paused, rx }, tx, cloned_paused)
    }

    async fn run(&mut self) {
        debug!["starting custom handler"];
        while let Some(input) = self.rx.recv().await {
            let resp = match input.command {
                Command::Pause(Some(value)) => {
                    debug!["Called custom pause handler with paused={}", value];
                    self.is_paused.store(value, Release);
                    Ok(HandlerOutput::Ok)
                }
                Command::Status => {
                    debug!["Called custom status handler"];
                    Ok(HandlerOutput::from(CustomStatus {
                        volume: Some(20),
                        state: PlaybackStatus::Pause,
                    }))
                }
                Command::Stats => {
                    debug!["Called custom stats handler"];
                    Ok(HandlerOutput::Fields(vec![
                        ("one".to_string(), "1".to_string()),
                        ("two".to_string(), "2".to_string()),
                    ]))
                }
                _ => Err(HandlerError::Unsupported),
            };
            match input.resp.send(resp) {
                Ok(_) => {}
                Err(err) => {
                    warn!["Cannot send response: {:?}", err];
                }
            }
        }
    }
}

struct Client {
    stream: TcpStream,
}

impl Client {
    async fn new(address: String) -> Self {
        let stream = TcpStream::connect(address)
            .await
            .expect("Could not connect");
        let mut me = Self { stream };
        assert_eq!(
            b"OK MPD 0.21.25\n",
            me.read_bytes().await.as_str().as_bytes()
        );
        me
    }

    async fn read_bytes(&mut self) -> String {
        let mut read_buffer = [0; 1024];
        let read_or_timeout = timeout(
            Duration::from_millis(250),
            self.stream.read(&mut read_buffer),
        );
        let n = read_or_timeout
            .await
            .expect("Read timeout")
            .expect("Read error");
        let result = std::str::from_utf8(&read_buffer[0..n]).expect("Invalid UTF8");
        debug!("Read back result {:?}", result);
        result.to_string()
    }

    async fn send_command(&mut self, command: &str) {
        debug!("Sending command {:?}", command);
        self.stream
            .write(format!["{}\n", command].as_bytes())
            .await
            .expect("Error sending command");
    }

    async fn send_commands(&mut self, mut commands: Vec<&str>, verbose: bool) {
        debug!("Sending commands {:?}", commands);

        if verbose {
            commands.insert(0, "command_list_ok_begin");
        } else {
            commands.insert(0, "command_list_begin");
        }
        commands.push("command_list_end");

        for command in commands {
            self.stream
                .write(format!["{}\n", command].as_bytes())
                .await
                .expect("Error sending command");
        }
    }

    /// Check that no packet is waiting to be read
    async fn assert_no_response(&mut self) {
        let mut read_buffer = [0; 32];
        let read_or_timeout = timeout(
            Duration::from_millis(100),
            self.stream.read(&mut read_buffer),
        );
        match read_or_timeout.await {
            Err(_) => {} // Expected behaviour if waiting for next command
            Ok(Err(err)) => panic!("Read error {:?}", err),
            Ok(Ok(0)) => {} // Expected behaviour on close
            Ok(Ok(remaining)) => panic!("Found {} extra bytes", remaining),
        }
    }

    /// Reads the response and compare it to the expected one
    async fn assert_response(&mut self, expected: String) {
        // Wait for complete response and check equality
        let mut response = "".to_string();
        while expected.len() > response.len() {
            response.push_str(self.read_bytes().await.as_str());
        }
        assert_eq![response, expected];

        // Confirm there is nothing else on the wire
        self.assert_no_response().await;
    }
}

use mpdify::mpd::listener::Listener;
use std::io::{Read, Write};
use log::{debug, warn};
use std::time::Duration;
use mpdify::mpd::handlers::{HandlerInput, HandlerOutput, HandlerError};
use mpdify::mpd::commands::Command;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::{Release, Acquire};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn it_handles_two_connections() {
    init_logger();

    let mut listener = Listener::new("127.0.0.1:0".to_string(), vec![]).await;
    let address = listener.get_address().expect("Cannot get server address");

    debug!("Listening on random port {}", address);
    tokio::spawn(async move { listener.run().await });

    let mut clients = vec![
        Client::new(address.clone()).await,
        Client::new(address.clone()).await,
    ];

    for client in clients.iter_mut().rev() {
        client.send_command("ping").await;
        assert_eq!("OK\n", client.read_bytes().await);
    }

    for client in clients.iter_mut() {
        client.send_command("close").await;
        assert!(client.read_bytes().await.is_empty());
    }
}


#[tokio::test]
async fn it_calls_custom_handler() {
    init_logger();

    // Run custom handler
    let (pause_tx, mut pause_rx) = mpsc::channel(8);
    let handlers: Vec<Sender<HandlerInput>> = vec![pause_tx];
    let is_paused = Arc::new(AtomicBool::new(false));
    let cloned_paused = is_paused.clone();

    tokio::spawn(async move {
        debug!["starting custom handler"];
        while let Some(input) = pause_rx.recv().await {
            let resp = match input.command {
                Command::Pause(value) => {
                    debug!["Called custom pause handler with paused={}", value];
                    cloned_paused.store(value, Release);
                    Ok(HandlerOutput::Ok)
                }
                _ => Err(HandlerError::Unsupported),
            };
            match input.resp.send(resp) {
                Ok(_) => {},
                Err(err) => {
                    warn!["Cannot send response: {:?}", err];
                },
            }
        }
    });

    let mut listener = Listener::new("127.0.0.1:0".to_string(), handlers).await;
    let address = listener.get_address().expect("Cannot get server address");
    debug!("Listening on random port {}", address);
    tokio::spawn(async move { listener.run().await });

    let mut client = Client::new(address.clone()).await;

    // Pause command is sent to our handler
    client.send_command("pause 1").await;
    assert_eq!("OK\n", client.read_bytes().await);
    assert_eq!(true, is_paused.load(Acquire));
    client.send_command("pause 0").await;
    assert_eq!("OK\n", client.read_bytes().await);
    assert_eq!(false, is_paused.load(Acquire));

    // Ping is still handled by the default handler
    client.send_command("ping").await;
    assert_eq!("OK\n", client.read_bytes().await);
    client.send_command("close").await;
    assert!(client.read_bytes().await.is_empty());
}

fn init_logger() {
    let _ = pretty_env_logger::try_init();
}
struct Client { stream: TcpStream }

impl Client {
    async fn new(address: String) -> Self {
        let stream = TcpStream::connect(address).await.expect("Could not connect");
        Self { stream }
    }

    async fn read_bytes(&mut self) -> String {
        let mut read_buffer = [0; 16];
        let n = self.stream.read(&mut read_buffer).await.expect("Cannot read");
        let result = std::str::from_utf8(&read_buffer[0..n]).expect("Invalid UTF8");
        debug!("Read back result {:?}", result);
        result.to_string()
    }

    async fn send_command(&mut self, command: &str) {
        debug!("Sending command {:?}", command);
        self.stream.write(format!["{}\n", command].as_bytes()).await.expect("Error sending command");
    }
}
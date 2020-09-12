use log::{debug, warn};
use mpdify::mpd::commands::Command;
use mpdify::mpd::handlers::{HandlerError, HandlerInput, HandlerOutput};
use mpdify::mpd::listener::{MpdListener, MPD_HELLO_STRING};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::{Acquire, Release};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};

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
    assert_eq!("OK\n", client.read_bytes().await);
    assert_eq!(true, is_paused.load(Acquire));
    client.send_command("pause \"0\"").await;
    assert_eq!("OK\n", client.read_bytes().await);
    assert_eq!(false, is_paused.load(Acquire));

    // Status command is sent to our handler
    client.send_command("status").await;
    assert_eq!("one: 1\ntwo: 2\nOK\n", client.read_bytes().await);

    // Ping is still handled by the default handler
    client.send_command("ping").await;
    assert_eq!("OK\n", client.read_bytes().await);
    client.send_command("close").await;
    assert!(client.read_bytes().await.is_empty());
}

fn init_logger() {
    let _ = pretty_env_logger::try_init();
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
                    Ok(HandlerOutput::Data(vec![
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
        assert_eq!(MPD_HELLO_STRING, me.read_bytes().await.as_str().as_bytes());
        me
    }

    async fn read_bytes(&mut self) -> String {
        let mut read_buffer = [0; 32];
        let n = self
            .stream
            .read(&mut read_buffer)
            .await
            .expect("Cannot read");
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
}

use mpdify::mpd::listener::Listener;
use std::net::TcpStream;
use std::io::{Read, Write};
use log::debug;
use std::thread;
use std::time::Duration;
use mpdify::mpd::outputtypes::{CommandHandler, HandlerResult, HandlerOutput, HandlerError};
use mpdify::mpd::commands::Command;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::{Release, Acquire};
use std::sync::Arc;

#[test]
fn it_handles_two_connections() {
    init_logger();

    let listener = Listener::new("127.0.0.1:0".to_string(), vec![]);
    let address = listener.get_address().expect("Cannot get server address");
    debug!("Listening on random port {}", address);
    thread::spawn(move || { listener.run() });

    let mut clients = vec![
        Client::new(address.clone()),
        Client::new(address.clone()),
    ];

    for client in clients.iter_mut().rev() {
        client.send_command("ping");
        assert_eq!("OK\n", client.read_bytes());
    }

    for client in clients.iter_mut() {
        client.send_command("close");
        assert!(client.read_bytes().is_empty());
    }
}

#[test]
fn it_calls_custom_handler() {
    init_logger();

    struct CustomHandler { paused: Arc<AtomicBool> };

    impl CommandHandler for CustomHandler {
        fn handle(&self, command: &Command) -> HandlerResult {
            match command {
                Command::Pause(paused) => {
                    debug!["Called custom pause handler with paused={}", paused];
                    self.paused.store(*paused, Release);
                    Ok(HandlerOutput::Ok)
                }
                _ => Err(HandlerError::Unsupported)
            }
        }
    }

    let paused = Arc::new(AtomicBool::new(false));
    let listener = Listener::new(
        "127.0.0.1:0".to_string(),
        vec![Box::new(CustomHandler{paused: paused.clone() })]
    );
    let address = listener.get_address().expect("Cannot get server address");
    let mut client = Client::new(address.clone());
    debug!("Listening on random port {}", address);
    thread::spawn(move || { listener.run() });

    // Pause command is sent to our handler
    client.send_command("pause 1");
    assert_eq!("OK\n", client.read_bytes());
    assert_eq!(true, paused.load(Acquire));
    client.send_command("pause 0");
    assert_eq!("OK\n", client.read_bytes());
    assert_eq!(false, paused.load(Acquire));

    // Ping is still handled by the default handler
    client.send_command("ping");
    assert_eq!("OK\n", client.read_bytes());
}

fn init_logger() {
    let _ = pretty_env_logger::try_init();
}
struct Client { stream: TcpStream }

impl Client {
    fn new(address: String) -> Self {
        let timeout = Option::Some(Duration::from_millis(100));
        let stream = TcpStream::connect(address).expect("Could not connect");
        stream.set_write_timeout(timeout).expect("Error setting write timeout");
        stream.set_read_timeout(timeout).expect("Error setting read timeout");
        Self { stream }
    }

    fn read_bytes(&mut self) -> String {
        let mut read_buffer = [0; 16];
        let n = self.stream.read(&mut read_buffer).expect("Cannot read");
        let result = std::str::from_utf8(&read_buffer[0..n]).expect("Invalid UTF8");
        debug!("Read back result {:?}", result);
        result.to_string()
    }

    fn send_command(&mut self, command: &str) {
        debug!("Sending command {:?}", command);
        write!(self.stream, "{}", command).expect("Error sending command");
    }
}
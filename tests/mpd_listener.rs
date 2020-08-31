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
        init_client(address.clone()),
        init_client(address.clone())
    ];

    for c in clients.iter_mut().rev() {
        debug!("Writing ping");
        write!(c, "ping").expect("Error sending ping");

        debug!("Reading reply");
        assert_eq!("OK\n", read_bytes(c));
    }

    for c in clients.iter_mut() {
        debug!("Writing close");
        write!(c, "close").expect("Error sending close");

        debug!("Reading reply");
        assert_eq!("", read_bytes(c));
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
    let mut client = init_client(address.clone());
    debug!("Listening on random port {}", address);
    thread::spawn(move || { listener.run() });

    // Pause command is sent to our handler
    write!(client, "pause 1").expect("Error sending command");
    assert_eq!("OK\n", read_bytes(&mut client));
    assert_eq!(true, paused.load(Acquire));
    write!(client, "pause 0").expect("Error sending command");
    assert_eq!("OK\n", read_bytes(&mut client));
    assert_eq!(false, paused.load(Acquire));

    // Ping is still handled by the default handler
    write!(client, "ping").expect("Error sending command");
    assert_eq!("OK\n", read_bytes(&mut client));
}

fn init_logger() {
    let _ = pretty_env_logger::try_init();
}

fn init_client(address: String) -> TcpStream {
    let timeout = Option::Some(Duration::from_millis(100));
    let client = TcpStream::connect(address).expect("Could not connect");
    client.set_write_timeout(timeout).expect("Error setting write timeout");
    client.set_read_timeout(timeout).expect("Error setting read timeout");
    client
}

fn read_bytes(reader: &mut dyn Read) -> String {
    let mut read_buffer = [0; 16];
    let n = reader.read(&mut read_buffer).expect("Cannot read");
    std::str::from_utf8(&read_buffer[0..n]).expect("Invalid UTF8").to_string()
}
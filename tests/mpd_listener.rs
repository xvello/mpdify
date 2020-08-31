use mpdify::mpd::listener::Listener;
use std::net::TcpStream;
use std::io::{Read, Write};
use log::debug;
use std::thread;
use std::time::Duration;

const TIMEOUTS: Duration = Duration::from_millis(100);

#[test]
fn it_handles_two_connections() {
    pretty_env_logger::init();

    let listener = Listener::new("127.0.0.1:0".to_string());
    let address = listener.get_address().expect("Cannot get server address");
    debug!("Listening on random port {}", address);
    thread::spawn(move || { listener.run() });

    let mut clients = vec![
        TcpStream::connect(address.clone()).expect("Could not connect first client"),
        TcpStream::connect(address.clone()).expect("Could not connect second client")
    ];

    for c in clients.iter_mut().rev() {
        debug!("Setting timeouts");
        c.set_write_timeout(Option::Some(TIMEOUTS)).expect("Error setting write timeout");
        c.set_read_timeout(Option::Some(TIMEOUTS)).expect("Error setting read timeout");

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

fn read_bytes(reader: &mut dyn Read) -> String {
    let mut read_buffer = [0; 16];
    let n = reader.read(&mut read_buffer).expect("Cannot read");
    std::str::from_utf8(&read_buffer[0..n]).expect("Invalid UTF8").to_string()
}
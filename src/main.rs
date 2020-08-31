use mpdify::mpd::listener::Listener;

fn main() {
    println!("Hello, world!");
    pretty_env_logger::init();

    let listener = Listener::new("0.0.0.0:6600".to_string(), vec![]);
    listener.run();
}

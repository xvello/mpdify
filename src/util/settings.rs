use config::{Config, ConfigError, Environment};
use serde::Deserialize;
use std::net::{IpAddr, SocketAddr};

#[derive(Debug, Deserialize)]
pub struct Settings {
    mpd_port: u16,
    http_port: u16,
    http_host: String,
    bind_address: IpAddr,
    pub playback_pool_freq_base_seconds: u64,
    pub playback_pool_freq_fast_seconds: u64,
}

impl Settings {
    fn init() -> Result<Config, ConfigError> {
        let mut s = Config::new();
        s.set_default("mpd_port", 6600)?;
        s.set_default("http_port", 6601)?;
        s.set_default("http_host", "localhost")?;
        s.set_default("bind_address", "0.0.0.0")?;
        s.set_default("playback_pool_freq_base_seconds", "15")?;
        s.set_default("playback_pool_freq_fast_seconds", "1")?;
        Ok(s)
    }

    /// Parses settings from environment variables
    pub fn new() -> Result<Self, ConfigError> {
        let mut s = Settings::init()?;
        s.merge(Environment::with_prefix("mpdify"))?;
        s.try_into()
    }

    /// Combines defaults with provided values, for tests
    pub fn with(source: Config) -> Result<Self, ConfigError> {
        let mut s = Settings::init()?;
        s.merge(source)?;
        s.try_into()
    }

    pub fn auth_path(&self) -> String {
        format!["http://{}:{}/auth", self.http_host, self.http_port]
    }

    pub fn http_address(&self) -> SocketAddr {
        SocketAddr::new(self.bind_address, self.http_port)
    }

    pub fn mpd_address(&self) -> SocketAddr {
        SocketAddr::new(self.bind_address, self.mpd_port)
    }
}

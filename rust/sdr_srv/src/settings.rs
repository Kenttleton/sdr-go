use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub server: ServerSettings,
    pub logging: LoggingSettings,
    pub device: DeviceSettings,
}

#[derive(Debug, Deserialize)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct LoggingSettings {
    /// Log level: trace | debug | info | warn | error
    pub level: String,
    /// Output format: pretty | compact | json
    pub format: String,
}

#[derive(Debug, Deserialize)]
pub struct DeviceSettings {
    /// How to locate the hardware: stub | first_available | index | serial | vid_pid
    pub source: String,
    /// Used when source = "index"
    #[serde(default)]
    pub index: Option<usize>,
    /// Used when source = "serial"
    #[serde(default)]
    pub serial: Option<String>,
    /// USB vendor ID hex string, used when source = "vid_pid" (e.g. "0bda")
    #[serde(default)]
    pub vid: Option<String>,
    /// USB product ID hex string, used when source = "vid_pid" (e.g. "2838")
    #[serde(default)]
    pub pid: Option<String>,
    /// Initial center frequency in Hz
    pub center_hz: u32,
    /// RTL-SDR IQ sample rate in samples/sec
    pub sample_rate: u32,
    /// Audio output sample rate in Hz
    pub audio_rate: u32,
    /// Tuner gain in tenths of dB (e.g. 200 = 20.0 dB); absent = auto-gain
    #[serde(default)]
    pub gain_tenths: Option<i32>,
    /// Enable bias-tee (5 V on antenna port for active antennas)
    pub bias_tee: bool,
}

impl AppConfig {
    /// Load config from (in priority order, lowest → highest):
    ///   1. built-in defaults
    ///   2. `config.toml` in the working directory (optional)
    ///   3. `SDR__*` environment variables (double-underscore separator)
    ///      e.g. SDR__SERVER__PORT=9090, SDR__DEVICE__SOURCE=first_available
    pub fn load() -> Result<Self, ConfigError> {
        Config::builder()
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 8080)?
            .set_default("logging.level", "info")?
            .set_default("logging.format", "pretty")?
            .set_default("device.source", "stub")?
            .set_default("device.center_hz", 101_100_000)?
            .set_default("device.sample_rate", 2_400_000)?
            .set_default("device.audio_rate", 48_000)?
            .set_default("device.bias_tee", false)?
            .add_source(File::with_name("config").required(false))
            .add_source(Environment::with_prefix("SDR").separator("__"))
            .build()?
            .try_deserialize()
    }
}

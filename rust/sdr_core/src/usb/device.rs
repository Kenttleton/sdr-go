use rtl_sdr_rs::{DeviceId, RtlSdr, TunerGain};
use std::{sync::Arc, error::Error};
use parking_lot::Mutex;

#[derive(Debug, Clone)]
pub struct DeviceConfig {
    pub frequency_hz: u32,
    pub sample_rate: u32,
    pub gain_db: Option<f32>, // None = auto gain
    pub bias_tee: bool,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            frequency_hz: 100_000_000, // 100 MHz default
            sample_rate: 2_048_000,    // 2.048 MSPS — standard RTL-SDR rate
            gain_db: None,             // auto gain
            bias_tee: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("Failed to open device: {0}")]
    OpenFailed(String),
    #[error("Failed to set frequency: {0}")]
    FrequencyError(String),
    #[error("Failed to set sample rate: {0}")]
    SampleRateError(String),
    #[error("Device not open")]
    NotOpen,
}

pub struct SdrDevice {
    inner: Arc<Mutex<Option<RtlSdr>>>,
    config: DeviceConfig,
}

impl SdrDevice {
    /// Open device from Android file descriptor passed via JNI
    pub fn open_from_fd(fd: i32, config: DeviceConfig) -> Result<Self, DeviceError> {
        let sdr = RtlSdr::open(DeviceId::Fd(fd))
            .map_err(|e| DeviceError::OpenFailed(e.to_string()))?;

        let device = Self {
            inner: Arc::new(Mutex::new(Some(sdr))),
            config: config.clone(),
        };

        device.apply_config(&config)?;
        Ok(device)
    }

    pub fn set_frequency(&mut self, freq_hz: u32) -> Result<(), DeviceError> {
        let mut guard = self.inner.lock();
        let sdr = guard.as_mut().ok_or(DeviceError::NotOpen)?;
        sdr.set_center_freq(freq_hz)
            .map_err(|e| DeviceError::FrequencyError(e.to_string()))?;
        self.config.frequency_hz = freq_hz;
        Ok(())
    }

    pub fn set_sample_rate(&mut self, rate: u32) -> Result<(), DeviceError> {
        let mut guard = self.inner.lock();
        let sdr = guard.as_mut().ok_or(DeviceError::NotOpen)?;
        sdr.set_sample_rate(rate)
            .map_err(|e| DeviceError::SampleRateError(e.to_string()))?;
        self.config.sample_rate = rate;
        Ok(())
    }

    pub fn config(&self) -> &DeviceConfig {
        &self.config
    }

    fn apply_config(&self, config: &DeviceConfig) -> Result<(), DeviceError> {
        let mut guard = self.inner.lock();
        let sdr = guard.as_mut().ok_or(DeviceError::NotOpen)?;

        sdr.set_center_freq(config.frequency_hz)
            .map_err(|e| DeviceError::FrequencyError(e.to_string()))?;

        sdr.set_sample_rate(config.sample_rate)
            .map_err(|e| DeviceError::SampleRateError(e.to_string()))?;

        match config.gain_db {
            Some(gain) => sdr.set_tuner_gain(TunerGain::Manual(gain as i32)),
            None => sdr.set_tuner_gain(TunerGain::Auto),
        }.map_err(|e| DeviceError::OpenFailed(e.to_string()))?;

        Ok(())
    }

    /// Clone the inner Arc for stream access
    pub(crate) fn inner(&self) -> Arc<Mutex<Option<RtlSdr>>> {
        Arc::clone(&self.inner)
    }
}
use super::hardware::{HardwareError, RtlSdrHardware, SdrHardware};
use libusb1_sys::{constants::*, *};
use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct DeviceConfig {
    pub frequency_hz: u32,
    pub sample_rate: u32,
    pub gain_tenths: Option<i32>,
    pub bias_tee: bool,
    pub audio_sample_rate: u32,
}

// Probe the USB bulk IN endpoint (0x81) to determine the natural transfer size for the device on this connection.
fn probe_bulk_transfer_samples(fd: i32) -> usize {
    const FALLBACK: usize = 8_192;
    unsafe {
        let mut ctx = std::ptr::null_mut();
        if libusb_init(&mut ctx) != LIBUSB_SUCCESS {
            return FALLBACK;
        }

        let mut handle = std::ptr::null_mut();
        let r = libusb_wrap_sys_device(ctx, fd as isize as *mut std::os::raw::c_int, &mut handle);
        if r != LIBUSB_SUCCESS {
            libusb_exit(ctx);
            return FALLBACK;
        }

        let device = libusb_get_device(handle);
        let packet_bytes = libusb_get_max_packet_size(device, 0x81);
        let speed = libusb_get_device_speed(device);

        libusb_close(handle);
        libusb_exit(ctx);

        if packet_bytes <= 0 {
            return FALLBACK;
        }

        // Batch packets per transfer, targeting ~32 KB, tuned by USB speed class.
        let batch = match speed {
            LIBUSB_SPEED_FULL => 256,
            LIBUSB_SPEED_HIGH => 64,
            LIBUSB_SPEED_SUPER | LIBUSB_SPEED_SUPER_PLUS => 32,
            _ => 64,
        };

        let total_bytes = (packet_bytes as usize) * batch;
        let samples = total_bytes / 2;
        log::info!(
            "probe_bulk_transfer_samples: speed={} packet_bytes={} batch={} → {} IQ samples",
            speed,
            packet_bytes,
            batch,
            samples
        );
        samples
    }
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            frequency_hz: 100_000_000,
            sample_rate: 2_048_000,
            gain_tenths: None,
            bias_tee: false,
            audio_sample_rate: 96_000,
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
    #[error("Failed to set gain: {0}")]
    GainError(String),
    #[error("Failed to read samples: {0}")]
    ReadError(String),
    #[error("Device not open")]
    NotOpen,
}

impl From<HardwareError> for DeviceError {
    fn from(e: HardwareError) -> Self {
        match e {
            HardwareError::OpenError(s) => DeviceError::OpenFailed(s),
            HardwareError::FrequencyError(s) => DeviceError::FrequencyError(s),
            HardwareError::SampleRateError(s) => DeviceError::SampleRateError(s),
            HardwareError::GainError(s) => DeviceError::GainError(s),
            HardwareError::ReadError(s) => DeviceError::ReadError(s),
        }
    }
}

pub struct SdrDevice {
    inner: Arc<Mutex<Option<Box<dyn SdrHardware>>>>,
    config: DeviceConfig,
    bulk_transfer_samples: usize,
}

impl SdrDevice {
    pub fn open_from_fd(fd: i32, config: DeviceConfig) -> Result<Self, DeviceError> {
        log::info!("SdrDevice: opening fd={}", fd);
        let bulk_transfer_samples = probe_bulk_transfer_samples(fd);
        let mut hw = RtlSdrHardware::open(fd)?;

        hw.set_center_freq(config.frequency_hz)?;
        log::info!("SdrDevice: tuned to {} Hz", config.frequency_hz);

        hw.set_sample_rate(config.sample_rate)?;
        log::info!("SdrDevice: sample rate {} sps", config.sample_rate);

        hw.set_tuner_gain(config.gain_tenths)?;
        log::info!("SdrDevice: gain {:?}", config.gain_tenths);

        Ok(Self {
            inner: Arc::new(Mutex::new(Some(Box::new(hw)))),
            config,
            bulk_transfer_samples,
        })
    }

    pub fn from_hardware(hw: Box<dyn SdrHardware>, config: DeviceConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Some(hw))),
            config,
            bulk_transfer_samples: 8_192,
        }
    }

    pub fn bulk_transfer_samples(&self) -> usize {
        self.bulk_transfer_samples
    }

    pub fn set_frequency(&mut self, freq_hz: u32) -> Result<(), DeviceError> {
        let mut guard = self.inner.lock();
        let hw = guard.as_mut().ok_or(DeviceError::NotOpen)?;
        hw.set_center_freq(freq_hz)?;
        self.config.frequency_hz = freq_hz;
        Ok(())
    }

    pub fn set_sample_rate(&mut self, rate: u32) -> Result<(), DeviceError> {
        let mut guard = self.inner.lock();
        let hw = guard.as_mut().ok_or(DeviceError::NotOpen)?;
        hw.set_sample_rate(rate)?;
        self.config.sample_rate = rate;
        Ok(())
    }

    pub fn set_gain(&mut self, tenths_db: i32, auto_gain: bool) -> Result<(), DeviceError> {
        let mut guard = self.inner.lock();
        let hw = guard.as_mut().ok_or(DeviceError::NotOpen)?;
        hw.set_tuner_gain(if auto_gain { None } else { Some(tenths_db) })?;
        self.config.gain_tenths = if auto_gain { None } else { Some(tenths_db) };
        Ok(())
    }

    pub fn available_gains(&self) -> Vec<i32> {
        let guard = self.inner.lock();
        match guard.as_ref() {
            Some(hw) => hw.available_tuner_gains().unwrap_or_default(),
            None => vec![],
        }
    }

    pub fn config(&self) -> &DeviceConfig {
        &self.config
    }

    pub fn close(&mut self) {
        let mut guard = self.inner.lock();
        *guard = None;
    }

    pub(crate) fn inner(&self) -> Arc<Mutex<Option<Box<dyn SdrHardware>>>> {
        Arc::clone(&self.inner)
    }
}

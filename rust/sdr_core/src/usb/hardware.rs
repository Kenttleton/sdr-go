use std::sync::Once;

use num_complex::Complex;
use rtl_sdr_rs::{DeviceId, RtlSdr, TunerGain};

#[derive(Debug, thiserror::Error)]
pub enum HardwareError {
    #[error("Failed to open device: {0}")]
    OpenError(String),
    #[error("Failed to set frequency: {0}")]
    FrequencyError(String),
    #[error("Failed to set sample rate: {0}")]
    SampleRateError(String),
    #[error("Failed to set gain: {0}")]
    GainError(String),
    #[error("Failed to read samples: {0}")]
    ReadError(String),
}

pub trait SdrHardware: Send {
    fn set_center_freq(&mut self, freq_hz: u32) -> Result<(), HardwareError>;
    fn set_sample_rate(&mut self, rate: u32) -> Result<(), HardwareError>;
    fn set_gain(&mut self, tenths_db: i32, auto_gain: bool) -> Result<(), HardwareError>;
    /// Read up to `num_samples` IQ samples, normalized to [-1.0, 1.0].
    fn read_samples(&mut self, num_samples: usize) -> Result<Vec<Complex<f32>>, HardwareError>;
    fn available_gains(&self) -> Vec<i32>;
}

// Fallback Gains Table (RTL2832U)
pub const R820T2_GAINS_TENTHS: &[i32] = &[
    0, 9, 14, 27, 37, 77, 87, 125, 144, 157, 166, 197, 207, 229, 254, 280, 297, 328, 338, 364, 372,
    386, 402, 421, 434, 439, 445, 480, 496,
];

pub struct RtlSdrHardware {
    sdr: RtlSdr,
    gains: Vec<i32>,
}

// SAFETY: RtlSdr contains Box<dyn Tuner> which is not auto-Send.
unsafe impl Send for RtlSdrHardware {}

// Must be called once before any rtl-sdr-rs context is created on Android.
// Without this, libusb tries to enumerate /dev/bus/usb/ (which fails on Android),
// and the empty device list causes claim_interface to return LIBUSB_ERROR_IO even
// though libusb_wrap_sys_device succeeded.
static LIBUSB_ANDROID_INIT: Once = Once::new();

fn init_libusb_android() {
    LIBUSB_ANDROID_INIT.call_once(|| unsafe {
        libusb1_sys::libusb_set_option(
            std::ptr::null_mut(),
            libusb1_sys::constants::LIBUSB_OPTION_NO_DEVICE_DISCOVERY,
        );
    });
}

impl RtlSdrHardware {
    pub fn open(fd: i32) -> Result<Self, HardwareError> {
        init_libusb_android();
        let sdr =
            RtlSdr::open(DeviceId::Fd(fd)).map_err(|e| HardwareError::OpenError(e.to_string()))?;
        let gains = sdr
            .get_tuner_gains()
            .unwrap_or_else(|_| R820T2_GAINS_TENTHS.to_vec());
        let tuner_id = sdr.get_tuner_id().unwrap_or("unknown");
        log::info!("RtlSdrHardware: tuner={} gains={:?}", tuner_id, gains);
        let hw = Self { sdr, gains };
        hw.sdr
            .reset_buffer()
            .map_err(|e| HardwareError::OpenError(format!("reset_buffer: {}", e)))?;
        log::info!("RtlSdrHardware: reset_buffer OK — ready to stream");
        Ok(hw)
    }
}

// Note: Implement SdrHardware to add a new backend (HackRF, Airspy, SDRplay, …).
impl SdrHardware for RtlSdrHardware {
    fn set_center_freq(&mut self, freq_hz: u32) -> Result<(), HardwareError> {
        self.sdr
            .set_center_freq(freq_hz)
            .map_err(|e| HardwareError::FrequencyError(e.to_string()))
    }

    fn set_sample_rate(&mut self, rate: u32) -> Result<(), HardwareError> {
        self.sdr
            .set_sample_rate(rate)
            .map_err(|e| HardwareError::SampleRateError(e.to_string()))
    }

    fn set_gain(&mut self, tenths_db: i32, auto_gain: bool) -> Result<(), HardwareError> {
        let mode = if auto_gain {
            TunerGain::Auto
        } else {
            TunerGain::Manual(tenths_db)
        };
        self.sdr
            .set_tuner_gain(mode)
            .map_err(|e| HardwareError::GainError(e.to_string()))
    }

    fn read_samples(&mut self, num_samples: usize) -> Result<Vec<Complex<f32>>, HardwareError> {
        let mut raw = vec![0u8; num_samples * 2];
        let bytes_read = self
            .sdr
            .read_sync(&mut raw)
            .map_err(|e| HardwareError::ReadError(e.to_string()))?;
        let samples = raw[..bytes_read]
            .chunks_exact(2)
            .map(|c| Complex::new((c[0] as f32 - 127.5) / 127.5, (c[1] as f32 - 127.5) / 127.5))
            .collect();
        Ok(samples)
    }

    fn available_gains(&self) -> Vec<i32> {
        self.gains.clone()
    }
}

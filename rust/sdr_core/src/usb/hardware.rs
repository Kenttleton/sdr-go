use std::sync::Once;

use num_complex::Complex;
use rtl_sdr_rs::{DeviceDescriptor, DeviceId, RtlSdr, TunerGain};

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
    fn get_center_freq(&self) -> u32;

    fn set_sample_rate(&mut self, rate: u32) -> Result<(), HardwareError>;
    fn get_sample_rate(&self) -> u32;

    fn set_tuner_gain(&mut self, tenths_db: Option<i32>) -> Result<(), HardwareError>;
    fn get_tuner_gain(&self) -> Result<i32, HardwareError>;
    fn available_tuner_gains(&self) -> Result<Vec<i32>, HardwareError>;

    fn set_tuner_bandwidth(&mut self, bw: u32) -> Result<(), HardwareError>;
    fn get_tuner_id(&self) -> &str;
    fn set_bias_tee(&self, on: bool) -> Result<(), HardwareError>;

    fn read_samples(&mut self, num_samples: usize) -> Result<Vec<Complex<f32>>, HardwareError>;
}

// Fallback Gains Table (RTL2832U)
pub const R820T2_GAINS_TENTHS: &[i32] = &[
    0, 9, 14, 27, 37, 77, 87, 125, 144, 157, 166, 197, 207, 229, 254, 280, 297, 328, 338, 364,
    372, 386, 402, 421, 434, 439, 445, 480, 496,
];

pub struct RtlSdrHardware {
    id: String,
    bias_tee_on: bool,
    sdr: RtlSdr,
    gains: Vec<i32>,
}

// SAFETY: RtlSdr contains Box<dyn Tuner> which is not auto-Send.
unsafe impl Send for RtlSdrHardware {}

#[cfg(unix)]
static LIBUSB_NO_DISCOVERY: Once = Once::new();

/// Suppress libusb bus enumeration for the lifetime of the process.
/// Required on Android where /dev/bus/usb/ is not accessible; harmless on other
/// Unix platforms when you know you will only ever use FD-based opens.
#[cfg(unix)]
fn set_no_device_discovery() {
    LIBUSB_NO_DISCOVERY.call_once(|| unsafe {
        libusb1_sys::libusb_set_option(
            std::ptr::null_mut(),
            libusb1_sys::constants::LIBUSB_OPTION_NO_DEVICE_DISCOVERY,
        );
    });
}

fn finish_open(sdr: RtlSdr) -> Result<RtlSdrHardware, HardwareError> {
    let gains = sdr
        .get_tuner_gains()
        .unwrap_or_else(|_| R820T2_GAINS_TENTHS.to_vec());
    let tuner_id = sdr.get_tuner_id().unwrap_or("unknown").to_string();
    log::info!("RtlSdrHardware: tuner={} gains={:?}", tuner_id, gains);
    let hw = RtlSdrHardware { id: tuner_id, sdr, gains, bias_tee_on: false };
    hw.sdr
        .reset_buffer()
        .map_err(|e| HardwareError::OpenError(format!("reset_buffer: {}", e)))?;
    log::info!("RtlSdrHardware: reset_buffer OK — ready to stream");
    Ok(hw)
}

impl RtlSdrHardware {
    /// Open from a file descriptor (Unix/Android only).
    ///
    /// Set `no_discovery = true` when running on Android (or any platform where
    /// libusb cannot enumerate the USB bus). This applies
    /// LIBUSB_OPTION_NO_DEVICE_DISCOVERY for the process lifetime — call it
    /// before any other libusb context is created.
    ///
    /// Not available on Windows — rtl-sdr-rs gates `DeviceId::Fd` to `#[cfg(unix)]`.
    #[cfg(unix)]
    pub fn open_fd(fd: i32, no_discovery: bool) -> Result<Self, HardwareError> {
        if no_discovery {
            set_no_device_discovery();
        }
        let sdr = RtlSdr::open(DeviceId::Fd(fd))
            .map_err(|e| HardwareError::OpenError(e.to_string()))?;
        finish_open(sdr)
    }

    /// Open the first available RTL-SDR device. Requires the `enumerate` feature.
    #[cfg(feature = "enumerate")]
    pub fn open_first() -> Result<Self, HardwareError> {
        let sdr = RtlSdr::open_first_available()
            .map_err(|e| HardwareError::OpenError(e.to_string()))?;
        finish_open(sdr)
    }

    /// Open by device index. Requires the `enumerate` feature.
    #[cfg(feature = "enumerate")]
    pub fn open_index(index: usize) -> Result<Self, HardwareError> {
        let sdr = RtlSdr::open(DeviceId::Index(index))
            .map_err(|e| HardwareError::OpenError(e.to_string()))?;
        finish_open(sdr)
    }

    /// Open by serial number string. Requires the `enumerate` feature.
    #[cfg(feature = "enumerate")]
    pub fn open_serial(serial: &str) -> Result<Self, HardwareError> {
        let sdr = RtlSdr::open(DeviceId::Serial(serial))
            .map_err(|e| HardwareError::OpenError(e.to_string()))?;
        finish_open(sdr)
    }

    /// Open the first device matching a USB vendor/product ID pair.
    /// Requires the `enumerate` feature.
    #[cfg(feature = "enumerate")]
    pub fn open_vid_pid(vid: u16, pid: u16) -> Result<Self, HardwareError> {
        let devices = RtlSdr::list_devices()
            .map_err(|e| HardwareError::OpenError(format!("list_devices: {e}")))?;
        let descriptor = devices
            .into_iter()
            .find(|d| d.vendor_id == vid && d.product_id == pid)
            .ok_or_else(|| {
                HardwareError::OpenError(format!(
                    "no device with VID {:04x} PID {:04x}",
                    vid, pid
                ))
            })?;
        let sdr = RtlSdr::open_with_index(descriptor.index)
            .map_err(|e| HardwareError::OpenError(e.to_string()))?;
        finish_open(sdr)
    }

    /// List all RTL-SDR devices visible to libusb.
    /// Returns the rtl-sdr-rs `DeviceDescriptor` for each found device.
    /// Requires the `enumerate` feature.
    #[cfg(feature = "enumerate")]
    pub fn list_devices() -> Vec<DeviceDescriptor> {
        RtlSdr::list_devices().unwrap_or_default()
    }
}

// Note: implement SdrHardware to add a new backend (HackRF, Airspy, SDRplay, …).
impl SdrHardware for RtlSdrHardware {
    fn set_center_freq(&mut self, freq_hz: u32) -> Result<(), HardwareError> {
        self.sdr
            .set_center_freq(freq_hz)
            .map_err(|e| HardwareError::FrequencyError(e.to_string()))
    }

    fn get_center_freq(&self) -> u32 {
        self.sdr.get_center_freq()
    }

    fn set_sample_rate(&mut self, rate: u32) -> Result<(), HardwareError> {
        self.sdr
            .set_sample_rate(rate)
            .map_err(|e| HardwareError::SampleRateError(e.to_string()))
    }

    fn get_sample_rate(&self) -> u32 {
        self.sdr.get_sample_rate()
    }

    fn set_tuner_gain(&mut self, tenths_db: Option<i32>) -> Result<(), HardwareError> {
        let mode = if let Some(x) = tenths_db {
            TunerGain::Manual(x)
        } else {
            TunerGain::Auto
        };
        self.sdr
            .set_tuner_gain(mode)
            .map_err(|e| HardwareError::GainError(e.to_string()))
    }

    fn get_tuner_gain(&self) -> Result<i32, HardwareError> {
        Err(HardwareError::GainError("get_tuner_gain not supported".to_string()))
    }

    fn available_tuner_gains(&self) -> Result<Vec<i32>, HardwareError> {
        Ok(self.gains.clone())
    }

    fn set_tuner_bandwidth(&mut self, bw: u32) -> Result<(), HardwareError> {
        self.sdr
            .set_tuner_bandwidth(bw)
            .map_err(|e| HardwareError::GainError(e.to_string()))
    }

    fn get_tuner_id(&self) -> &str {
        &self.id
    }

    fn set_bias_tee(&self, on: bool) -> Result<(), HardwareError> {
        self.sdr
            .set_bias_tee(on)
            .map_err(|e| HardwareError::OpenError(e.to_string()))
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
}

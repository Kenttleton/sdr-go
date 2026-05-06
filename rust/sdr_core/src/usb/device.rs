use super::hardware::{HardwareError, RtlSdrHardware, SdrHardware};
use libusb1_sys::{constants::*, *};
use parking_lot::Mutex;
use std::sync::Arc;

// ── Device source ─────────────────────────────────────────────────────────────

/// Describes how to locate and open the hardware.
///
/// Open method and discovery mode are orthogonal:
/// - `FileDescriptor` works on any POSIX platform (Android, Linux, embedded).
/// - All other variants require USB device discovery via the `enumerate` feature
///   and a platform where libusb can access the bus (Linux, macOS).
pub enum DeviceSource {
    /// Open from an existing file descriptor (Unix/Android only).
    ///
    /// Set `no_discovery = true` on Android — Java hands you the FD and libusb
    /// cannot enumerate /dev/bus/usb/ there. On Linux you can open a device
    /// node yourself (e.g. /dev/bus/usb/001/003) with `no_discovery = false`.
    ///
    /// Not available on Windows — use an `enumerate` variant there instead.
    #[cfg(unix)]
    FileDescriptor { fd: i32, no_discovery: bool },

    /// Open the first available RTL-SDR device.
    /// Requires the `enumerate` feature.
    #[cfg(feature = "enumerate")]
    FirstAvailable,

    /// Open by device index (assigned by libusb enumeration order).
    /// Requires the `enumerate` feature.
    #[cfg(feature = "enumerate")]
    Index(usize),

    /// Open by USB serial number string.
    /// Useful for targeting a specific unit in a multi-device setup.
    /// Requires the `enumerate` feature.
    #[cfg(feature = "enumerate")]
    Serial(String),

    /// Open the first device matching a USB vendor ID / product ID pair.
    /// Useful when targeting a specific chip variant or manufacturer.
    /// Requires the `enumerate` feature.
    #[cfg(feature = "enumerate")]
    VidPid { vid: u16, pid: u16 },
}

impl DeviceSource {
    /// Convenience: Android — FD with discovery suppressed.
    #[cfg(unix)]
    pub fn android_fd(fd: i32) -> Self {
        Self::FileDescriptor { fd, no_discovery: true }
    }

    /// Convenience: POSIX FD with discovery left enabled (Linux device node, etc.).
    #[cfg(unix)]
    pub fn posix_fd(fd: i32) -> Self {
        Self::FileDescriptor { fd, no_discovery: false }
    }
}

// ── Device info ───────────────────────────────────────────────────────────────

/// Metadata about an enumerated RTL-SDR device.
#[cfg(feature = "enumerate")]
pub struct DeviceInfo {
    pub index: usize,
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: String,
    pub product: String,
    pub serial: String,
}

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DeviceConfig {
    pub frequency_hz: u32,
    pub sample_rate: u32,
    pub gain_tenths: Option<i32>,
    pub bias_tee: bool,
    pub audio_sample_rate: u32,
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

// ── Errors ────────────────────────────────────────────────────────────────────

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

// ── Bulk transfer probe ───────────────────────────────────────────────────────

/// Probe the USB bulk IN endpoint (0x81) via an open FD to find the natural
/// transfer size for this device/connection. Falls back to 8 192 on failure.
#[cfg(unix)]
fn probe_bulk_transfer_samples_fd(fd: i32) -> usize {
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
            speed, packet_bytes, batch, samples
        );
        samples
    }
}

// ── SdrDevice ─────────────────────────────────────────────────────────────────

pub struct SdrDevice {
    inner: Arc<Mutex<Option<Box<dyn SdrHardware>>>>,
    config: DeviceConfig,
    bulk_transfer_samples: usize,
}

impl SdrDevice {
    /// Primary constructor. Accepts any `DeviceSource`.
    pub fn open(source: DeviceSource, config: DeviceConfig) -> Result<Self, DeviceError> {
        let (hw, bulk_transfer_samples): (Box<dyn SdrHardware>, usize) = match source {
            #[cfg(unix)]
            DeviceSource::FileDescriptor { fd, no_discovery } => {
                log::info!("SdrDevice: opening fd={} no_discovery={}", fd, no_discovery);
                let bulk = probe_bulk_transfer_samples_fd(fd);
                let hw = RtlSdrHardware::open_fd(fd, no_discovery)?;
                (Box::new(hw), bulk)
            }

            #[cfg(feature = "enumerate")]
            DeviceSource::FirstAvailable => {
                log::info!("SdrDevice: opening first available device");
                (Box::new(RtlSdrHardware::open_first()?), 8_192)
            }

            #[cfg(feature = "enumerate")]
            DeviceSource::Index(index) => {
                log::info!("SdrDevice: opening by index={}", index);
                (Box::new(RtlSdrHardware::open_index(index)?), 8_192)
            }

            #[cfg(feature = "enumerate")]
            DeviceSource::Serial(ref serial) => {
                log::info!("SdrDevice: opening by serial={}", serial);
                (Box::new(RtlSdrHardware::open_serial(serial)?), 8_192)
            }

            #[cfg(feature = "enumerate")]
            DeviceSource::VidPid { vid, pid } => {
                log::info!("SdrDevice: opening by VID {:04x} PID {:04x}", vid, pid);
                (Box::new(RtlSdrHardware::open_vid_pid(vid, pid)?), 8_192)
            }
        };

        Self::configure(hw, bulk_transfer_samples, config)
    }

    fn configure(
        mut hw: Box<dyn SdrHardware>,
        bulk_transfer_samples: usize,
        config: DeviceConfig,
    ) -> Result<Self, DeviceError> {
        hw.set_center_freq(config.frequency_hz)?;
        log::info!("SdrDevice: tuned to {} Hz", config.frequency_hz);

        hw.set_sample_rate(config.sample_rate)?;
        log::info!("SdrDevice: sample rate {} sps", config.sample_rate);

        hw.set_tuner_gain(config.gain_tenths)?;
        log::info!("SdrDevice: gain {:?}", config.gain_tenths);

        Ok(Self {
            inner: Arc::new(Mutex::new(Some(hw))),
            config,
            bulk_transfer_samples,
        })
    }

    /// Wrap an already-constructed hardware backend (useful for testing).
    pub fn from_hardware(hw: Box<dyn SdrHardware>, config: DeviceConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Some(hw))),
            config,
            bulk_transfer_samples: 8_192,
        }
    }

    /// List all RTL-SDR devices visible on the USB bus.
    /// Requires the `enumerate` feature.
    #[cfg(feature = "enumerate")]
    pub fn enumerate() -> Vec<DeviceInfo> {
        RtlSdrHardware::list_devices()
            .into_iter()
            .map(|d| DeviceInfo {
                index: d.index,
                vendor_id: d.vendor_id,
                product_id: d.product_id,
                manufacturer: d.manufacturer,
                product: d.product,
                serial: d.serial,
            })
            .collect()
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
        *self.inner.lock() = None;
    }

    pub(crate) fn inner(&self) -> Arc<Mutex<Option<Box<dyn SdrHardware>>>> {
        Arc::clone(&self.inner)
    }

    /// Build an `IqStream` backed by this device. Consumes the device so the
    /// stream owns exclusive access to the hardware handle.
    pub fn into_stream(self) -> super::stream::IqStream {
        let bulk = self.bulk_transfer_samples;
        super::stream::IqStream::new(self.inner, bulk, None)
    }
}

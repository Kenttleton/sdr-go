use super::hardware::SdrHardware;
use num_complex::Complex;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static FILL_OK_COUNT: AtomicU64 = AtomicU64::new(0);
static FILL_ERR_COUNT: AtomicU64 = AtomicU64::new(0);

pub type IqSample = Complex<f32>;
pub type IqBuffer = Vec<IqSample>;

const RING_BUFFER_SIZE: usize = 1024 * 1024; // 1M samples

pub struct IqStream {
    inner: Arc<Mutex<Option<Box<dyn SdrHardware>>>>,
    ring: Vec<IqSample>,
    write_pos: usize,
    read_pos: usize,
    overflows: u64,
    read_size: usize,
    /// Samples remaining to discard after a hardware retune.
    settling_samples: usize,
}

impl IqStream {
    pub fn new(
        inner: Arc<Mutex<Option<Box<dyn SdrHardware>>>>,
        hardware_read_size: usize,
        override_size: Option<usize>,
    ) -> Self {
        let read_size = match override_size {
            None => hardware_read_size,
            Some(req) if req <= hardware_read_size => req,
            Some(req) => {
                log::warn!(
                    "IqStream: requested read_size={} exceeds hardware capacity={}, clamping",
                    req,
                    hardware_read_size
                );
                hardware_read_size
            }
        };
        log::info!(
            "IqStream: read_size={} hardware_capacity={} ring_size={}",
            read_size,
            hardware_read_size,
            RING_BUFFER_SIZE,
        );
        Self {
            inner,
            ring: vec![Complex::new(0.0, 0.0); RING_BUFFER_SIZE],
            write_pos: 0,
            read_pos: 0,
            overflows: 0,
            read_size,
            settling_samples: 0,
        }
    }

    pub fn fill(&mut self) -> Result<usize, String> {
        let mut guard = self.inner.lock();
        let hw = guard.as_mut().ok_or("Device not open")?;
        let samples = hw.read_samples(self.read_size).map_err(|e| {
            let n = FILL_ERR_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
            log::warn!("IqStream::fill error #{}: {}", n, e);
            e.to_string()
        })?;
        let ok_count = FILL_OK_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        let samples_written = samples.len();
        if ok_count <= 5 || ok_count % 500 == 0 {
            log::info!(
                "IqStream::fill #{}: {} samples, ring available={}",
                ok_count,
                samples_written,
                self.available()
            );
        }
        for sample in samples {
            let next = (self.write_pos + 1) % RING_BUFFER_SIZE;
            if next == self.read_pos {
                self.overflows += 1;
                self.read_pos = (self.read_pos + 1) % RING_BUFFER_SIZE;
            }
            self.ring[self.write_pos] = sample;
            self.write_pos = next;
        }
        // Discard post-retune settling samples as they arrive.
        if self.settling_samples > 0 {
            let to_skip = self.settling_samples.min(self.available());
            self.read_pos = (self.read_pos + to_skip) % RING_BUFFER_SIZE;
            self.settling_samples -= to_skip;
        }

        Ok(samples_written)
    }

    /// Signal that the hardware has just been retuned.
    /// The next ~100 ms of samples (204,800 @ 2.048 MSPS) are discarded while
    /// the R820T2 PLL and AGC re-settle.
    pub fn mark_retuned(&mut self) {
        self.settling_samples = 204_800;
    }

    pub fn drain(&mut self, count: usize) -> IqBuffer {
        let available = self.available();
        let to_read = count.min(available);
        let mut out = Vec::with_capacity(to_read);
        for _ in 0..to_read {
            out.push(self.ring[self.read_pos]);
            self.read_pos = (self.read_pos + 1) % RING_BUFFER_SIZE;
        }
        out
    }

    pub fn available(&self) -> usize {
        if self.write_pos >= self.read_pos {
            self.write_pos - self.read_pos
        } else {
            RING_BUFFER_SIZE - self.read_pos + self.write_pos
        }
    }

    pub fn overflows(&self) -> u64 {
        self.overflows
    }

    pub fn flush(&mut self) {
        self.read_pos = self.write_pos;
    }
}

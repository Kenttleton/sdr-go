use num_complex::Complex;
use std::sync::Arc;
use parking_lot::Mutex;
use rtl_sdr_rs::RtlSdr;

pub type IqSample = Complex<f32>;
pub type IqBuffer = Vec<IqSample>;

const RING_BUFFER_SIZE: usize = 1024 * 1024; // 1M samples
const READ_SIZE: usize = 16384;              // 16K samples per USB read

pub struct IqStream {
    inner: Arc<Mutex<Option<RtlSdr>>>,
    ring: Vec<IqSample>,
    write_pos: usize,
    read_pos: usize,
    overflows: u64,
}

impl IqStream {
    pub fn new(inner: Arc<Mutex<Option<RtlSdr>>>) -> Self {
        Self {
            inner,
            ring: vec![Complex::new(0.0, 0.0); RING_BUFFER_SIZE],
            write_pos: 0,
            read_pos: 0,
            overflows: 0,
        }
    }

    pub fn fill(&mut self) -> Result<usize, String> {
        let mut guard = self.inner.lock();
        let sdr = guard.as_mut().ok_or("Device not open")?;

        let mut raw = vec![0u8; READ_SIZE * 2]; // allocate buffer
        let bytes_read = sdr.read_sync(&mut raw)
            .map_err(|e| e.to_string())?;

        let samples_written = bytes_read / 2;

        for chunk in raw[..bytes_read].chunks_exact(2) {
            let i = (chunk[0] as f32 - 127.5) / 127.5;
            let q = (chunk[1] as f32 - 127.5) / 127.5;

            let next = (self.write_pos + 1) % RING_BUFFER_SIZE;
            if next == self.read_pos {
                self.overflows += 1;
                self.read_pos = (self.read_pos + 1) % RING_BUFFER_SIZE;
            }
            self.ring[self.write_pos] = Complex::new(i, q);
            self.write_pos = next;
        }

        Ok(samples_written)
    }

    /// Drain up to `count` samples from ring buffer
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
}
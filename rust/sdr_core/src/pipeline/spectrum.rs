use num_complex::Complex;
use rustfft::{num_complex::Complex as FftComplex, FftPlanner};

type Cf32 = Complex<f32>;

// ── FFT ───────────────────────────────────────────────────────────────────────

pub struct FftStage {
    planner: FftPlanner<f32>,
    pub size: usize,
}

impl FftStage {
    pub fn new(size: usize) -> Self {
        Self {
            planner: FftPlanner::new(),
            size,
        }
    }

    /// Returns `size/2` magnitude bins in dBFS, Hann-windowed.
    pub fn magnitude_spectrum(&mut self, samples: &[Cf32]) -> Vec<f32> {
        let size = self.size.min(samples.len());
        let fft = self.planner.plan_fft_forward(size);

        let mut buf: Vec<FftComplex<f32>> = samples[..size]
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let w =
                    0.5 * (1.0 - (std::f32::consts::TAU * i as f32 / (size - 1) as f32).cos());
                FftComplex::new(s.re * w, s.im * w)
            })
            .collect();

        fft.process(&mut buf);

        let half = size / 2;
        (0..half)
            .map(|i| {
                let shifted = (i + half) % size;
                let mag = buf[shifted].norm();
                if mag > 0.0 {
                    20.0 * mag.log10()
                } else {
                    -120.0
                }
            })
            .collect()
    }
}

// ── Waveform ──────────────────────────────────────────────────────────────────

/// Captures 512-sample display snapshots for two signal paths:
///   - IQ envelope: magnitude of raw RF samples, pre-gate, pre-demod
///   - Audio:       post-demod PCM samples
///
/// Both are updated opportunistically from data already in flight.
/// The frontend chooses which (if any) to display.
pub struct WaveformStage {
    iq_waveform: [f32; 512],
    iq_ready: bool,
    audio_waveform: [f32; 512],
    audio_ready: bool,
}

impl WaveformStage {
    pub fn new() -> Self {
        Self {
            iq_waveform: [0.0; 512],
            iq_ready: false,
            audio_waveform: [0.0; 512],
            audio_ready: false,
        }
    }

    /// Update the IQ envelope snapshot from raw IQ samples.
    /// Called with the same block passed to the demod pipeline.
    pub fn update_iq(&mut self, iq: &[Cf32]) {
        if iq.is_empty() {
            return;
        }
        let step = (iq.len() / 512).max(1);
        for (i, slot) in self.iq_waveform.iter_mut().enumerate() {
            let idx = (i * step).min(iq.len() - 1);
            let s = iq[idx];
            *slot = (s.re * s.re + s.im * s.im).sqrt();
        }
        self.iq_ready = true;
    }

    /// Update the audio waveform snapshot from demod PCM output.
    /// Strides through whatever PCM layout the active pipeline produces.
    pub fn update_audio(&mut self, pcm: &[f32]) {
        if pcm.is_empty() {
            return;
        }
        let step = (pcm.len() / 512).max(1);
        for (i, slot) in self.audio_waveform.iter_mut().enumerate() {
            let idx = (i * step).min(pcm.len() - 1);
            *slot = pcm[idx];
        }
        self.audio_ready = true;
    }

    /// Returns the IQ envelope snapshot and clears the ready flag.
    /// Returns None if not updated since the last call.
    pub fn take_iq_waveform(&mut self) -> Option<[f32; 512]> {
        if self.iq_ready {
            self.iq_ready = false;
            Some(self.iq_waveform)
        } else {
            None
        }
    }

    /// Returns the audio waveform snapshot and clears the ready flag.
    /// Returns None if not updated since the last call.
    pub fn take_audio_waveform(&mut self) -> Option<[f32; 512]> {
        if self.audio_ready {
            self.audio_ready = false;
            Some(self.audio_waveform)
        } else {
            None
        }
    }
}

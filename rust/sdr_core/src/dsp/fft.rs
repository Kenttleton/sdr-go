use rustfft::{FftPlanner, num_complex::Complex};

/// FFT wrapper for spectrum and waterfall display
/// Produces magnitude spectrum from IQ samples
pub struct Fft {
    planner: FftPlanner<f32>,
    size: usize,
}

impl Fft {
    pub fn new(size: usize) -> Self {
        Self {
            planner: FftPlanner::new(),
            size,
        }
    }

    /// Compute magnitude spectrum from IQ samples
    /// Returns Vec of length size/2 (positive frequencies only)
    /// Values are in dB relative to full scale
    pub fn magnitude_spectrum(
        &mut self,
        samples: &[num_complex::Complex<f32>],
    ) -> Vec<f32> {
        let size = self.size.min(samples.len());
        let fft = self.planner.plan_fft_forward(size);

        // Copy and window input to reduce spectral leakage
        let mut buffer: Vec<Complex<f32>> = samples[..size]
            .iter()
            .enumerate()
            .map(|(i, s)| {
                // Hann window
                let window = 0.5 * (1.0
                    - (2.0 * std::f32::consts::PI * i as f32
                        / (size - 1) as f32)
                        .cos());
                Complex::new(s.re * window, s.im * window)
            })
            .collect();

        fft.process(&mut buffer);

        // FFT shift and compute magnitude in dB
        // Positive frequencies only (size/2 bins)
        let half = size / 2;
        let mut magnitudes = vec![0.0f32; half];

        for i in 0..half {
            // Shift so DC is in the center
            let shifted = (i + half) % size;
            let mag = buffer[shifted].norm();
            // Convert to dB, floor at -120 dB
            magnitudes[i] = if mag > 0.0 {
                20.0 * mag.log10()
            } else {
                -120.0
            };
        }

        magnitudes
    }
}
use num_complex::Complex;

/// FIR filter — convolves input with a set of coefficients
/// Used for low-pass filtering before demodulation and decimation
pub struct FirFilter {
    coefficients: Vec<f32>,
    buffer: Vec<f32>,
    pos: usize,
}

impl FirFilter {
    pub fn new(coefficients: Vec<f32>) -> Self {
        let len = coefficients.len();
        Self {
            coefficients,
            buffer: vec![0.0; len],
            pos: 0,
        }
    }

    /// Windowed sinc low-pass filter
    /// cutoff: normalized frequency (0.0–0.5 relative to sample rate)
    /// taps: number of filter coefficients — more taps = sharper cutoff, more CPU
    pub fn low_pass(cutoff: f32, taps: usize) -> Self {
        let mut coeffs = vec![0.0f32; taps];
        let mid = (taps - 1) as f32 / 2.0;

        for (i, c) in coeffs.iter_mut().enumerate() {
            let x = i as f32 - mid;
            // Sinc function
            let sinc = if x == 0.0 {
                1.0
            } else {
                (std::f32::consts::PI * x * cutoff * 2.0).sin()
                    / (std::f32::consts::PI * x)
            };
            // Hamming window to reduce sidelobes
            let window = 0.54 - 0.46 * (2.0 * std::f32::consts::PI * i as f32
                / (taps - 1) as f32).cos();
            *c = sinc * window;
        }

        // Normalize so filter has unity gain at DC
        let sum: f32 = coeffs.iter().sum();
        coeffs.iter_mut().for_each(|c| *c /= sum);

        Self::new(coeffs)
    }

    /// Process a single real sample
    pub fn process(&mut self, sample: f32) -> f32 {
        self.buffer[self.pos] = sample;
        self.pos = (self.pos + 1) % self.buffer.len();

        let mut output = 0.0f32;
        let len = self.coefficients.len();
        for (i, &coeff) in self.coefficients.iter().enumerate() {
            let idx = (self.pos + i) % len;
            output += coeff * self.buffer[idx];
        }
        output
    }

    /// Process a block of real samples in place
    pub fn process_block(&mut self, samples: &mut [f32]) {
        for s in samples.iter_mut() {
            *s = self.process(*s);
        }
    }

    /// Process a block of complex IQ samples — filters I and Q independently
    pub fn process_iq(&mut self, samples: &mut Vec<Complex<f32>>) {
        for s in samples.iter_mut() {
            s.re = self.process(s.re);
            s.im = self.process(s.im);
        }
    }
}
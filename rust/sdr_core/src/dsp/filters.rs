use super::window::Window;
use num_complex::Complex;
use std::f32::consts::PI;

pub struct FIRFilter {
    coefficients: Vec<f32>,
    buffer_i: Vec<f32>,
    buffer_q: Vec<f32>,
    pos: usize,
}

impl FIRFilter {
    pub fn new(coefficients: Vec<f32>) -> Self {
        let len = coefficients.len();
        Self {
            coefficients,
            buffer_i: vec![0.0; len],
            buffer_q: vec![0.0; len],
            pos: 0,
        }
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

pub struct FilterSpec {
    pub taps: usize,
    pub window: Window,
}

impl FilterSpec {
    pub fn new(taps: usize, window: Window) -> Self {
        assert!(taps % 2 == 1, "Tap count must be odd for linear phase");
        Self { taps, window }
    }

    pub fn with_transition(
        transition_hz: f32,
        sample_rate: f32,
        attenuation_db: f32,
        window: Window,
    ) -> Self {
        Self::new(
            estimate_taps(transition_hz, sample_rate, attenuation_db),
            window,
        )
    }
}

pub fn design_lowpass(cutoff: f32, spec: &FilterSpec) -> Vec<f32> {
    let taps = spec.taps;
    let m = (taps - 1) as f32 / 2.0;

    let mut coeffs: Vec<f32> = (0..taps)
        .map(|i| {
            let n = i as f32 - m;
            if n == 0.0 {
                2.0 * cutoff
            } else {
                (2.0 * PI * cutoff * n).sin() / (PI * n)
            }
        })
        .collect();

    spec.window.apply(&mut coeffs);

    let sum: f32 = coeffs.iter().sum();
    coeffs.iter_mut().for_each(|c| *c /= sum);
    coeffs
}

pub fn design_highpass(cutoff: f32, spec: &FilterSpec) -> Vec<f32> {
    let mut coeffs = design_lowpass(cutoff, spec);
    spectral_inversion(&mut coeffs);
    coeffs
}

pub fn design_bandpass(low: f32, high: f32, spec: &FilterSpec) -> Vec<f32> {
    let lp_high = design_lowpass(high, spec);
    let lp_low = design_lowpass(low, spec);
    let mut coeffs: Vec<f32> = lp_high
        .iter()
        .zip(lp_low.iter())
        .map(|(h, l)| h - l)
        .collect();

    let center = (low + high) / 2.0;
    let gain: f32 = coeffs
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let n = i as f32 - (spec.taps - 1) as f32 / 2.0;
            c * (2.0 * PI * center * n).cos()
        })
        .sum::<f32>()
        .abs();
    if gain > 1e-10 {
        coeffs.iter_mut().for_each(|c| *c /= gain);
    }

    coeffs
}

pub fn design_bandstop(low: f32, high: f32, spec: &FilterSpec) -> Vec<f32> {
    let mut bp = design_bandpass(low, high, spec);
    spectral_inversion(&mut bp);
    bp
}

fn spectral_inversion(coeffs: &mut [f32]) {
    coeffs.iter_mut().for_each(|c| *c = -*c);
    coeffs[coeffs.len() / 2] += 1.0;
}

pub struct DecimatingFilter {
    filter: FIRFilter,
    decimation: usize,
    counter: usize,
}

impl DecimatingFilter {
    /// cutoff_hz: desired passband edge
    /// input_rate: incoming sample rate (e.g. 2_400_000)
    /// output_rate: desired output rate (e.g. 240_000)
    /// output_rate must divide input_rate evenly
    pub fn new(cutoff_hz: f32, input_rate: u32, output_rate: u32, window: Window) -> Self {
        let decimation = (input_rate / output_rate) as usize;
        assert_eq!(
            input_rate % output_rate,
            0,
            "output_rate must divide input_rate"
        );

        let cutoff_safe = cutoff_hz.min(0.9 * output_rate as f32 / 2.0);
        let cutoff_norm = normalize_freq(cutoff_safe, input_rate as f32);

        let transition_hz = output_rate as f32 / 2.0 - cutoff_safe;
        let spec = FilterSpec::with_transition(transition_hz, input_rate as f32, 80.0, window);

        Self {
            filter: FIRFilter::new(design_lowpass(cutoff_norm, &spec)),
            decimation,
            counter: 0,
        }
    }

    pub fn process(&mut self, input: &[Complex<f32>]) -> Vec<Complex<f32>> {
        let mut out = Vec::with_capacity(input.len() / self.decimation);
        for &sample in input {
            let filtered = self.filter.process_iq(sample);
            self.counter += 1;
            if self.counter >= self.decimation {
                self.counter = 0;
                out.push(filtered);
            }
        }
        out
    }
}

pub struct Biquad {
    // Feed-forward (numerator) coefficients
    b0: f32,
    b1: f32,
    b2: f32,
    // Feed-back (denominator) coefficients
    a1: f32,
    a2: f32,
    // Delay line (state)
    w1: f32,
    w2: f32,
}

impl Biquad {
    pub fn new(b0: f32, b1: f32, b2: f32, a1: f32, a2: f32) -> Self {
        Self {
            b0,
            b1,
            b2,
            a1,
            a2,
            w1: 0.0,
            w2: 0.0,
        }
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.w1;
        self.w1 = self.b1 * x - self.a1 * y + self.w2;
        self.w2 = self.b2 * x - self.a2 * y;
        y
    }

    pub fn process_block(&mut self, samples: &mut [f32]) {
        for s in samples.iter_mut() {
            *s = self.process(*s);
        }
    }

    pub fn reset(&mut self) {
        self.w1 = 0.0;
        self.w2 = 0.0;
    }
}

/// Cascade of biquad sections.
/// An Nth-order filter = N/2 biquads (rounded up).
pub struct IIRFilter {
    sections: Vec<Biquad>,
}

impl IIRFilter {
    pub fn new(sections: Vec<Biquad>) -> Self {
        Self { sections }
    }

    pub fn process(&mut self, x: f32) -> f32 {
        self.sections.iter_mut().fold(x, |s, bq| bq.process(s))
    }

    pub fn process_block(&mut self, samples: &mut [f32]) {
        for s in samples.iter_mut() {
            *s = self.process(*s);
        }
    }

    /// For IQ data — same coefficients, independent state per channel
    pub fn process_complex(&mut self, x: Complex<f32>) -> Complex<f32> {
        // Can't use the same state for I and Q — need separate filters
        // See IIRFilterStereo below
        unimplemented!("Use IIRFilterDual for complex/stereo")
    }

    pub fn reset(&mut self) {
        self.sections.iter_mut().for_each(|s| s.reset());
    }
}

/// Two IIRFilters sharing coefficients but with independent state.
/// Use this for IQ or stereo audio.
pub struct IIRFilterDual {
    i_channel: IIRFilter,
    q_channel: IIRFilter,
}

impl IIRFilterDual {
    pub fn new(sections: Vec<(f32, f32, f32, f32, f32)>) -> Self {
        let make = |s: &Vec<_>| {
            IIRFilter::new(
                s.iter()
                    .map(|&(b0, b1, b2, a1, a2)| Biquad::new(b0, b1, b2, a1, a2))
                    .collect(),
            )
        };
        Self {
            i_channel: make(&sections),
            q_channel: make(&sections),
        }
    }

    pub fn process(&mut self, x: Complex<f32>) -> Complex<f32> {
        Complex::new(self.i_channel.process(x.re), self.q_channel.process(x.im))
    }

    pub fn process_block(&mut self, samples: &mut [Complex<f32>]) {
        for s in samples.iter_mut() {
            *s = self.process(*s);
        }
    }
}

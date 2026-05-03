use num_complex::Complex;
use std::f32::consts::TAU;

type Cf32 = Complex<f32>;

/// Digital Down-Converter: multiplies IQ samples by a complex exponential to
/// shift the centre frequency by `offset_hz` without touching hardware.
///
/// Usable range: ±1 MHz at 2.048 MSPS before aliasing becomes significant.
/// Outside that range, trigger a hardware retune instead.
pub struct Ddc {
    phase: f32,
    phase_inc: f32,
    offset_hz: f32,
    sample_rate: f32,
}

impl Ddc {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            phase: 0.0,
            phase_inc: 0.0,
            offset_hz: 0.0,
            sample_rate,
        }
    }

    pub fn set_offset(&mut self, offset_hz: f32) {
        self.offset_hz = offset_hz;
        self.phase_inc = TAU * offset_hz / self.sample_rate;
        if offset_hz == 0.0 {
            self.phase = 0.0;
        }
    }

    pub fn offset_hz(&self) -> f32 {
        self.offset_hz
    }

    /// Shift `samples` in-place. No-op when offset is zero.
    pub fn process(&mut self, samples: &mut [Cf32]) {
        if self.phase_inc == 0.0 {
            return;
        }
        for s in samples.iter_mut() {
            *s *= Cf32::new(self.phase.cos(), self.phase.sin());
            self.phase = (self.phase + self.phase_inc).rem_euclid(TAU);
        }
    }
}

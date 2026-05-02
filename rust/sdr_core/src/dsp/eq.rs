use std::f32::consts::PI;

struct BiquadFilter {
    b0: f32, b1: f32, b2: f32,
    a1: f32, a2: f32,
    x1: f32, x2: f32,
    y1: f32, y2: f32,
}

impl BiquadFilter {
    fn peaking(freq_hz: f32, gain_db: f32, q: f32, sample_rate: f32) -> Self {
        let mut f = Self { b0: 0.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0,
                          x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0 };
        f.update_coeffs(freq_hz, gain_db, q, sample_rate);
        f
    }

    /// Update coefficients without touching filter state (x1/x2/y1/y2).
    /// Avoids the transient click that would occur if we zeroed the history.
    fn update_coeffs(&mut self, freq_hz: f32, gain_db: f32, q: f32, sample_rate: f32) {
        let a = 10.0f32.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * freq_hz / sample_rate;
        let cos_w0 = w0.cos();
        let alpha = w0.sin() / (2.0 * q);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;

        self.b0 = b0 / a0; self.b1 = b1 / a0; self.b2 = b2 / a0;
        self.a1 = a1 / a0; self.a2 = a2 / a0;
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
              - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1; self.x1 = x;
        self.y2 = self.y1; self.y1 = y;
        y
    }
}

/// Center frequencies for the 7-band EQ, matching EQ_BANDS in RdsModule.ts
/// SUB, BASS, MUD, MID, EDGE, PRES, AIR
const BAND_FREQS: [f32; 7] = [60.0, 250.0, 500.0, 1_000.0, 2_000.0, 10_000.0, 16_000.0];
const BAND_Q: f32 = 1.41; // ~1 octave bandwidth

pub struct Equalizer {
    filters_l: [BiquadFilter; 7],
    filters_r: [BiquadFilter; 7],
    sample_rate: f32,
}

impl Equalizer {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            filters_l: std::array::from_fn(|i| {
                BiquadFilter::peaking(BAND_FREQS[i], 0.0, BAND_Q, sample_rate)
            }),
            filters_r: std::array::from_fn(|i| {
                BiquadFilter::peaking(BAND_FREQS[i], 0.0, BAND_Q, sample_rate)
            }),
            sample_rate,
        }
    }

    /// Update all band gains without resetting filter state.
    pub fn set_bands(&mut self, gains_db: &[f32; 7]) {
        for i in 0..7 {
            self.filters_l[i].update_coeffs(BAND_FREQS[i], gains_db[i], BAND_Q, self.sample_rate);
            self.filters_r[i].update_coeffs(BAND_FREQS[i], gains_db[i], BAND_Q, self.sample_rate);
        }
    }

    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let mut lout = l;
        let mut rout = r;
        for i in 0..7 {
            lout = self.filters_l[i].process(lout);
            rout = self.filters_r[i].process(rout);
        }
        (lout, rout)
    }
}

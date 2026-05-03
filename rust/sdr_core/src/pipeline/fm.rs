use super::filters::{firdes, DecimatingFirFilter, FirFilter};
use num_complex::Complex;

type Cf32 = Complex<f32>;

pub struct FmPipeline {
    // Stage 1: discriminator state
    prev_sample: Cf32,

    // Stage 2: pre-filter + decimate (input_rate → ~200 kHz)
    pre_filter: DecimatingFirFilter,

    // Stage 3: pilot tracking (19 kHz NCO)
    pilot_phase: f32,
    pilot_phase_inc: f32,
    pilot_amplitude: f32,
    stereo_detected: bool,

    // Stage 4 / 5: sum and diff LPFs at intermediate rate
    audio_lpf: FirFilter,
    diff_lpf: FirFilter,

    // Stage 6: de-emphasis IIR state (runs at output_rate)
    deemph_l: f32,
    deemph_r: f32,
    deemph_alpha: f32,

    // Stage 6: AGC
    agc_gain: f32,
    agc_frozen: bool,

    diff_decimation: usize,

    stereo: bool,
}

impl FmPipeline {
    pub fn new(input_rate: u32, output_rate: u32, stereo: bool) -> Self {
        let pre_decimation = ((input_rate / 200_000) as usize).max(1);
        let intermediate_rate = input_rate / pre_decimation as u32;

        let pre_cutoff = 100_000.0 / input_rate as f32;
        let pre_taps = firdes::lowpass(pre_cutoff, firdes::Kaiser::new(50.0));
        let pre_filter = DecimatingFirFilter::new(pre_decimation, pre_taps);

        let audio_cutoff = 15_000.0 / intermediate_rate as f32;
        let audio_taps = firdes::lowpass(audio_cutoff, firdes::Kaiser::new(40.0));
        let audio_lpf = FirFilter::new(audio_taps);

        let diff_cutoff = 15_000.0 / intermediate_rate as f32;
        let diff_taps = firdes::lowpass(diff_cutoff, firdes::Kaiser::new(40.0));
        let diff_lpf = FirFilter::new(diff_taps);

        let diff_decimation = ((intermediate_rate / output_rate) as usize).max(1);

        let deemph_alpha = 1.0 - (-1.0_f32 / (75e-6 * output_rate as f32)).exp();
        let pilot_phase_inc = 2.0 * std::f32::consts::PI * 19_000.0 / intermediate_rate as f32;

        Self {
            prev_sample: Cf32::new(1.0, 0.0),
            pre_filter,
            pilot_phase: 0.0,
            pilot_phase_inc,
            pilot_amplitude: 0.0,
            stereo_detected: false,
            audio_lpf,
            diff_lpf,
            deemph_l: 0.0,
            deemph_r: 0.0,
            deemph_alpha,
            agc_gain: 1.0,
            agc_frozen: false,
            diff_decimation,
            stereo,
        }
    }

    /// Returns interleaved stereo float PCM: [L0, R0, L1, R1, …].
    pub fn process_iq(&mut self, iq: &[Cf32]) -> Vec<f32> {
        // Stage 1: polar discriminator
        let mut demod_buf = Vec::with_capacity(iq.len());
        for &s in iq {
            let product = s * self.prev_sample.conj();
            demod_buf.push(product.im.atan2(product.re));
            self.prev_sample = s;
        }

        // Stage 2: pre-filter + decimate → intermediate rate
        let disc_dec = self.pre_filter.process(&demod_buf);

        if disc_dec.is_empty() {
            return Vec::new();
        }

        // Stage 3: pilot tracking — produce per-sample subcarrier reference
        let mut subcarrier_refs = Vec::with_capacity(disc_dec.len());
        for &d in &disc_dec {
            let pilot_ref = self.pilot_phase.cos();
            let correlation = d * pilot_ref;
            self.pilot_amplitude = self.pilot_amplitude * 0.9999 + correlation.abs() * 0.0001;
            self.stereo_detected = self.pilot_amplitude > 0.05;
            self.pilot_phase =
                (self.pilot_phase + self.pilot_phase_inc).rem_euclid(std::f32::consts::TAU);
            subcarrier_refs.push((self.pilot_phase * 2.0).cos());
        }

        // Stage 4: sum LPF (L+R mono signal) at intermediate rate
        let sum_buf = self.audio_lpf.process(&disc_dec);

        // Stage 5: diff LPF (L−R stereo difference) at intermediate rate
        let diff_buf = if self.stereo_detected && self.stereo {
            let mixed: Vec<f32> = disc_dec
                .iter()
                .zip(&subcarrier_refs)
                .map(|(&d, &r)| d * r)
                .collect();
            self.diff_lpf.process(&mixed)
        } else {
            vec![0.0; sum_buf.len()]
        };

        // Stage 6: decimate to audio rate, de-emphasis, AGC, interleave
        let out_len = sum_buf.len() / self.diff_decimation;
        let mut left = Vec::with_capacity(out_len);
        let mut right = Vec::with_capacity(out_len);

        for i in (0..sum_buf.len()).step_by(self.diff_decimation) {
            let (l_raw, r_raw) = if self.stereo_detected && self.stereo {
                (sum_buf[i] + diff_buf[i], sum_buf[i] - diff_buf[i])
            } else {
                (sum_buf[i], sum_buf[i])
            };

            self.deemph_l += self.deemph_alpha * (l_raw - self.deemph_l);
            self.deemph_r += self.deemph_alpha * (r_raw - self.deemph_r);

            let gl = self.deemph_l * self.agc_gain;
            let gr = self.deemph_r * self.agc_gain;

            if !self.agc_frozen {
                let amp = gl.abs().max(gr.abs());
                self.agc_gain *= if amp > 0.5 { 1.0 - 0.001 } else { 1.0 + 0.0001 };
                self.agc_gain = self.agc_gain.clamp(0.001, 1000.0);
            }

            left.push(gl);
            right.push(gr);
        }

        left.iter()
            .zip(right.iter())
            .flat_map(|(&l, &r)| [l, r])
            .collect()
    }

    pub fn freeze_agc(&mut self, frozen: bool) {
        self.agc_frozen = frozen;
    }

    /// Enable or disable stereo decode.
    /// Enabling is rejected if no pilot tone is currently detected — returns false.
    pub fn set_stereo(&mut self, enabled: bool) -> bool {
        if enabled && !self.stereo_detected {
            return false;
        }
        self.stereo = enabled;
        true
    }

    pub fn is_stereo_detected(&self) -> bool {
        self.stereo_detected
    }
}

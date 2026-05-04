use super::filters::{firdes, DecimatingFirFilter, FirFilter};
use num_complex::Complex;

type Cf32 = Complex<f32>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FmMode {
    /// Wide FM — broadcast band, up to 200 kHz deviation, stereo pilot, RDS-capable.
    Wide,
    /// Narrow FM — voice/utility bands, ±5 kHz deviation, mono, no de-emphasis.
    Narrow,
}

pub struct FmPipeline {
    mode: FmMode,
    // Center of the demodulated audio channel in Hz (0 = DC, standard FM).
    // Non-zero shifts the audio band for sub-channel extraction (future use).
    #[allow(dead_code)]
    center_hz: f32,

    // Stage 1: discriminator state
    prev_sample: Cf32,

    // Stage 2: pre-filter + decimate
    //   WFM: input_rate → ~200 kHz (preserves 19 kHz pilot + 38 kHz stereo subcarrier)
    //   NFM: input_rate → ~4× half_bw (scales with channel bandwidth)
    pre_filter: DecimatingFirFilter,

    // Stage 3: pilot tracking (19 kHz NCO) — WFM only
    pilot_phase: f32,
    pilot_phase_inc: f32,
    pilot_amplitude: f32,
    stereo_detected: bool,

    // Stage 4 / 5: sum and diff LPFs at intermediate rate
    audio_lpf: FirFilter,
    diff_lpf: FirFilter,

    // Stage 6: de-emphasis IIR state (runs at output_rate)
    // NFM: alpha = 1.0 → IIR collapses to passthrough (no de-emphasis)
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
    /// Create a new FM pipeline.
    ///
    /// - `bandwidth_hz`: channel half-bandwidth in Hz; `None` uses the mode default
    ///   (WFM = 100 kHz, NFM = 12.5 kHz). Drives the pre-filter cutoff and intermediate rate.
    /// - `center_hz`: center of the demodulated audio channel relative to DC. `0.0` is
    ///   correct for all standard FM use. Non-zero reserved for future sub-band extraction.
    pub fn new(
        input_rate: u32,
        output_rate: u32,
        stereo: bool,
        mode: FmMode,
        bandwidth_hz: Option<f32>,
        center_hz: f32,
    ) -> Self {
        let half_bw = bandwidth_hz.unwrap_or(match mode {
            FmMode::Wide   => 100_000.0,
            FmMode::Narrow =>  12_500.0,
        });

        let audio_bw = match mode {
            FmMode::Wide   => 15_000.0f32,
            FmMode::Narrow =>  3_500.0f32,
        };

        // WFM needs a fixed ~200 kHz intermediate to preserve the 19 kHz pilot and
        // 38 kHz stereo subcarrier. NFM scales to 4× the channel half-bandwidth.
        let pre_target = match mode {
            FmMode::Wide   => 200_000u32,
            FmMode::Narrow => ((half_bw * 4.0) as u32).max(50_000),
        };
        let pre_decimation = ((input_rate / pre_target) as usize).max(1);
        let intermediate_rate = input_rate / pre_decimation as u32;

        let pre_filter = DecimatingFirFilter::new(
            pre_decimation,
            firdes::lowpass(half_bw / input_rate as f32, firdes::Kaiser::new(50.0)),
        );

        let audio_lpf = FirFilter::new(
            firdes::lowpass(audio_bw / intermediate_rate as f32, firdes::Kaiser::new(40.0)),
        );

        // diff LPF is only exercised in the WFM stereo path.
        let diff_lpf = FirFilter::new(
            firdes::lowpass(15_000.0 / intermediate_rate as f32, firdes::Kaiser::new(40.0)),
        );

        let diff_decimation = ((intermediate_rate / output_rate) as usize).max(1);

        // NFM: alpha = 1.0 → de-emphasis IIR is a passthrough (no pre-emphasis in voice FM).
        let deemph_alpha = match mode {
            FmMode::Wide   => 1.0 - (-1.0_f32 / (75e-6 * output_rate as f32)).exp(),
            FmMode::Narrow => 1.0,
        };

        let pilot_phase_inc =
            2.0 * std::f32::consts::PI * 19_000.0 / intermediate_rate as f32;

        Self {
            mode,
            center_hz,
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

        // Stage 3: pilot tracking — WFM only. NFM is always mono so we skip the NCO
        // entirely and leave stereo_detected = false, producing an empty subcarrier_refs
        // that the diff path below never touches (gated on stereo_detected && stereo).
        let mut subcarrier_refs = Vec::with_capacity(disc_dec.len());
        if self.mode == FmMode::Wide {
            for &d in &disc_dec {
                let pilot_ref = self.pilot_phase.cos();
                let correlation = d * pilot_ref;
                self.pilot_amplitude = self.pilot_amplitude * 0.9999 + correlation.abs() * 0.0001;
                self.stereo_detected = self.pilot_amplitude > 0.05;
                self.pilot_phase =
                    (self.pilot_phase + self.pilot_phase_inc).rem_euclid(std::f32::consts::TAU);
                subcarrier_refs.push((self.pilot_phase * 2.0).cos());
            }
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

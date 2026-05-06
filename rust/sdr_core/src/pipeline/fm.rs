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
    channel_half_bw_hz: f32,
    // Center of the demodulated audio channel in Hz (0 = DC, standard FM).
    // Non-zero shifts the audio band for sub-channel extraction (future use).
    #[allow(dead_code)]
    center_hz: f32,

    sample_rate: u32,

    // Stage 1: discriminator state
    prev_sample: Cf32,

    // Stage 2: pre-filter + decimate
    //   WFM: sample_rate → ~200 kHz (preserves 19 kHz pilot + 38 kHz stereo subcarrier)
    //   NFM: sample_rate → ~4× half_bw (scales with channel bandwidth)
    pre_filter: DecimatingFirFilter,

    // Stage 3: pilot tracking (19 kHz NCO) — WFM only
    pilot_phase: f32,
    pilot_phase_inc: f32,
    stereo_detected: bool,
    goertzel: Goertzel,
    goertzel_count: usize,

    // Stage 4 / 5: sum and diff LPFs at intermediate rate
    audio_lpf: FirFilter,
    diff_lpf: FirFilter,

    // Stage 6: DC blocker (NFM only) — removes carrier frequency offset artifacts.
    // y[n] = x[n] - x[n-1] + R·y[n-1], R ≈ 0.995 (cutoff ~150 Hz at 48 kHz)
    dc_x_prev: f32,
    dc_y_prev: f32,

    // Stage 6: de-emphasis IIR state (runs at output_rate)
    // NFM: alpha = 1.0 → IIR collapses to passthrough (no de-emphasis)
    deemph_l: f32,
    deemph_r: f32,
    deemph_alpha: f32,

    // Stage 6: AGC
    agc_gain: f32,
    agc_frozen: bool,

    diff_decimation: usize,
    output_rate: u32,

    stereo: bool,

    rssi_db: f32,
    squelch_db: f32,
    squelch_hang_samples: usize,
    squelch_hang_remaining: usize,

    // Reusable scratch buffers — cleared and refilled each call to avoid per-block allocation.
    demod_buf: Vec<f32>,
    disc_dec: Vec<f32>,
    subcarrier_refs: Vec<f32>,
    scratch: Vec<f32>,
    sum_buf: Vec<f32>,
    diff_buf: Vec<f32>,
}

impl FmPipeline {
    /// Create a new FM pipeline.
    ///
    /// - `bandwidth_hz`: channel half-bandwidth in Hz; `None` uses the mode default
    ///   (WFM = 100 kHz, NFM = 12.5 kHz). Drives the pre-filter cutoff and intermediate rate.
    /// - `center_hz`: center of the demodulated audio channel relative to DC. `0.0` is
    ///   correct for all standard FM use. Non-zero reserved for future sub-band extraction.
    pub fn new(
        sample_rate: u32,
        output_rate: u32,
        stereo: bool,
        mode: FmMode,
        channel_half_bw_hz: Option<f32>,
        center_hz: f32,
    ) -> Self {
        let channel_half_bw_hz = channel_half_bw_hz.unwrap_or(match mode {
            FmMode::Wide => 100_000.0,
            FmMode::Narrow => 12_500.0,
        });

        let audio_bw = match mode {
            FmMode::Wide => 15_000.0f32,
            FmMode::Narrow => 3_500.0f32,
        };

        // WFM needs a fixed ~200 kHz intermediate to preserve the 19 kHz pilot and
        // 38 kHz stereo subcarrier. NFM scales to 4× the channel half-bandwidth.
        let pre_target = match mode {
            FmMode::Wide => 200_000u32,
            FmMode::Narrow => ((channel_half_bw_hz * 4.0) as u32).max(50_000),
        };
        let pre_decimation = ((sample_rate / pre_target) as usize).max(1);
        let intermediate_rate = sample_rate / pre_decimation as u32;

        let pre_filter = DecimatingFirFilter::new(
            pre_decimation,
            firdes::lowpass(channel_half_bw_hz / sample_rate as f32, firdes::Kaiser::new(50.0)),
        );

        let audio_lpf = FirFilter::new(firdes::lowpass(
            audio_bw / intermediate_rate as f32,
            firdes::Kaiser::new(40.0),
        ));

        // diff LPF is only exercised in the WFM stereo path.
        let diff_lpf = FirFilter::new(firdes::lowpass(
            15_000.0 / intermediate_rate as f32,
            firdes::Kaiser::new(40.0),
        ));

        let diff_decimation = ((intermediate_rate / output_rate) as usize).max(1);

        // NFM: alpha = 1.0 → de-emphasis IIR is a passthrough (no pre-emphasis in voice FM).
        let deemph_alpha = match mode {
            FmMode::Wide => 1.0 - (-1.0_f32 / (75e-6 * output_rate as f32)).exp(),
            FmMode::Narrow => 1.0,
        };

        let pilot_phase_inc = 2.0 * std::f32::consts::PI * 19_000.0 / intermediate_rate as f32;

        Self {
            mode,
            channel_half_bw_hz,
            center_hz,
            sample_rate,
            prev_sample: Cf32::new(1.0, 0.0),
            pre_filter,
            pilot_phase: 0.0,
            pilot_phase_inc,
            stereo_detected: false,
            goertzel: Goertzel::new(19_000.0, intermediate_rate as f32),
            goertzel_count: 0,
            audio_lpf,
            diff_lpf,
            deemph_l: 0.0,
            deemph_r: 0.0,
            deemph_alpha,
            dc_x_prev: 0.0,
            dc_y_prev: 0.0,
            agc_gain: 1.0,
            agc_frozen: false,
            diff_decimation,
            output_rate,
            stereo,
            rssi_db: -100.0,
            squelch_db: f32::NEG_INFINITY,
            squelch_hang_samples: 0,
            squelch_hang_remaining: 0,
            demod_buf: Vec::new(),
            disc_dec: Vec::new(),
            subcarrier_refs: Vec::new(),
            scratch: Vec::new(),
            sum_buf: Vec::new(),
            diff_buf: Vec::new(),
        }
    }

    /// Writes interleaved stereo float PCM [L0, R0, L1, R1, …] into `out`.
    pub fn process_iq(&mut self, iq: &[Cf32], out: &mut Vec<f32>) {
        // RSSI — instantaneous in-band power in dBFS
        let power = iq.iter().map(|c| c.norm_sqr()).sum::<f32>() / iq.len() as f32;
        self.rssi_db = 10.0 * power.max(1e-10).log10();

        // Squelch gate — skip DSP entirely when signal is absent and hang has expired
        if self.rssi_db < self.squelch_db {
            if self.squelch_hang_remaining == 0 {
                out.clear();
                return;
            }
        } else {
            self.squelch_hang_remaining = self.squelch_hang_samples;
        }

        // Stage 1: polar discriminator
        self.demod_buf.clear();
        let deviation_hz = match self.mode {
            FmMode::Wide => 75_000.0_f32,
            FmMode::Narrow => 5_000.0_f32,
        };
        let k = self.sample_rate as f32 / (2.0 * std::f32::consts::PI * deviation_hz);
        for &s in iq {
            let product = s * self.prev_sample.conj();
            self.demod_buf.push(k * product.im.atan2(product.re));
            self.prev_sample = s;
        }

        // Stage 2: pre-filter + decimate → intermediate rate
        self.pre_filter.process_into(&self.demod_buf, &mut self.disc_dec);

        if self.disc_dec.is_empty() {
            out.clear();
            return;
        }

        // Stage 3: pilot tracking — WFM only. NFM is always mono so we skip the NCO
        // entirely and leave stereo_detected = false, producing an empty subcarrier_refs
        // that the diff path below never touches (gated on stereo_detected && stereo).
        self.subcarrier_refs.clear();
        if self.mode == FmMode::Wide {
            const WINDOW: usize = 1024;
            // Normalized threshold: power / N² > this ≈ pilot amplitude > ~0.05
            const THRESHOLD: f32 = 0.0006;
            for i in 0..self.disc_dec.len() {
                let d = self.disc_dec[i];
                self.goertzel.process(d);
                self.goertzel_count += 1;
                self.pilot_phase =
                    (self.pilot_phase + self.pilot_phase_inc).rem_euclid(std::f32::consts::TAU);
                self.subcarrier_refs.push((self.pilot_phase * 2.0).cos());

                if self.goertzel_count >= WINDOW {
                    let p = self.goertzel.power() / (WINDOW * WINDOW) as f32;
                    self.stereo_detected = p > THRESHOLD;
                    self.goertzel.reset();
                    self.goertzel_count = 0;
                }
            }
        }

        // Stage 4: sum LPF (L+R mono signal) at intermediate rate
        self.audio_lpf.process_into(&self.disc_dec, &mut self.sum_buf);

        // Stage 5: diff LPF (L−R stereo difference) at intermediate rate
        if self.stereo_detected && self.stereo {
            self.scratch.clear();
            self.scratch.extend(
                self.disc_dec.iter().zip(self.subcarrier_refs.iter()).map(|(&d, &r)| d * r),
            );
            self.diff_lpf.process_into(&self.scratch, &mut self.diff_buf);
        } else {
            self.diff_buf.resize(self.sum_buf.len(), 0.0);
            self.diff_buf.iter_mut().for_each(|x| *x = 0.0);
        }

        // Stage 6: decimate to audio rate, de-emphasis, AGC, write interleaved into out
        let out_frames = self.sum_buf.len() / self.diff_decimation;
        out.clear();
        out.reserve(out_frames * 2);

        for i in (0..self.sum_buf.len()).step_by(self.diff_decimation) {
            let (l_raw, r_raw) = if self.stereo_detected && self.stereo {
                (self.sum_buf[i] + self.diff_buf[i], self.sum_buf[i] - self.diff_buf[i])
            } else {
                (self.sum_buf[i], self.sum_buf[i])
            };

            let (l_raw, r_raw) = if self.mode == FmMode::Narrow {
                let y = l_raw - self.dc_x_prev + 0.995 * self.dc_y_prev;
                self.dc_x_prev = l_raw;
                self.dc_y_prev = y;
                (y, y)
            } else {
                (l_raw, r_raw)
            };

            self.deemph_l += self.deemph_alpha * (l_raw - self.deemph_l);
            self.deemph_r += self.deemph_alpha * (r_raw - self.deemph_r);

            let gl = self.deemph_l * self.agc_gain;
            let gr = self.deemph_r * self.agc_gain;

            if !self.agc_frozen {
                let target = 0.3;
                let amp = gl.abs().max(gr.abs());
                if amp > 1e-4 {
                    let rate = if amp > target { 0.02 } else { 0.005 };
                    self.agc_gain *= 1.0 + (target / amp - 1.0) * rate;
                    self.agc_gain = self.agc_gain.clamp(0.001, 1000.0);
                }
            }

            out.push(soft_clip(gl));
            out.push(soft_clip(gr));
        }

        // Drain hang counter by the number of stereo frames just produced
        self.squelch_hang_remaining =
            self.squelch_hang_remaining.saturating_sub(out.len() / 2);
    }

    pub fn channel_half_bw_hz(&self) -> f32 {
        self.channel_half_bw_hz
    }

    pub fn rssi_db(&self) -> f32 {
        self.rssi_db
    }

    /// `threshold_db`: open squelch above this level; `f32::NEG_INFINITY` disables.
    /// `hang_ms`: how long to keep the gate open after signal drops below threshold.
    pub fn set_squelch(&mut self, threshold_db: f32, hang_ms: f32) {
        self.squelch_db = threshold_db;
        self.squelch_hang_samples =
            (hang_ms * self.output_rate as f32 / 1000.0).round() as usize;
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

fn soft_clip(x: f32) -> f32 {
    x / (1.0 + x.abs())
}

struct Goertzel {
    coeff: f32,
    s_prev: f32,
    s_prev2: f32,
}

impl Goertzel {
    fn new(freq: f32, sample_rate: f32) -> Self {
        let w = 2.0 * std::f32::consts::PI * freq / sample_rate;
        Self {
            coeff: 2.0 * w.cos(),
            s_prev: 0.0,
            s_prev2: 0.0,
        }
    }

    fn process(&mut self, x: f32) {
        let s = x + self.coeff * self.s_prev - self.s_prev2;
        self.s_prev2 = self.s_prev;
        self.s_prev = s;
    }

    fn power(&self) -> f32 {
        self.s_prev2.powi(2) + self.s_prev.powi(2) - self.coeff * self.s_prev * self.s_prev2
    }

    fn reset(&mut self) {
        self.s_prev = 0.0;
        self.s_prev2 = 0.0;
    }
}

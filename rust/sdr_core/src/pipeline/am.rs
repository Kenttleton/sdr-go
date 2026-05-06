use super::filters::{firdes, ComplexDecimatingFirFilter, FirFilter};
use num_complex::Complex;

type Cf32 = Complex<f32>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AmMode {
    Envelope,
    Sam, // stub — Synchronous AM (PLL-based)
    Usb, // stub — Upper Sideband
    Lsb, // stub — Lower Sideband
    Dsb, // stub — Double Sideband
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AmBandwidth {
    Wide,   // 8 kHz — music, strong signals
    Normal, // 5 kHz — standard broadcast
    Narrow, // 3 kHz — interference / adjacent channel
    Voice,  // 2.5 kHz — severe interference / SSB
}

impl AmBandwidth {
    pub fn cutoff_hz(self) -> f32 {
        match self {
            Self::Wide => 8_000.0,
            Self::Normal => 5_000.0,
            Self::Narrow => 3_000.0,
            Self::Voice => 2_500.0,
        }
    }
}

pub struct AmPipeline {
    mode: AmMode,

    // Center of the demodulated audio channel in Hz.
    // DSB/envelope: 0 (carrier is at DC after demod).
    // USB: positive offset — BFO placed above the carrier (future product detector).
    // LSB: negative offset — BFO placed below the carrier (future product detector).
    #[allow(dead_code)]
    center_hz: f32,

    // Stage 1: IQ anti-alias + decimate (input_rate → ~51.2 kHz)
    pre_filter: ComplexDecimatingFirFilter,
    pre_decimation: usize,
    intermediate_rate: u32,

    // Stage 3: DC blocking IIR — removes carrier residual after envelope detect
    dc_state: f32,
    dc_alpha: f32,

    // Stage 4: variable-bandwidth audio IF filter
    // Stored as Hz so set_bandwidth_hz() can compare without going through the enum.
    audio_filter: FirFilter,
    bandwidth_hz: f32,

    // Stage 5: AGC
    agc_gain: f32,
    agc_frozen: bool,

    output_rate: u32,

    rssi_db: f32,
    squelch_db: f32,
    squelch_hang_samples: usize,
    squelch_hang_remaining: usize,

    decimated_buf: Vec<Cf32>,
    dc_blocked_buf: Vec<f32>,
    filtered_buf: Vec<f32>,
}

impl AmPipeline {
    /// Create a new AM pipeline.
    ///
    /// - `bandwidth_hz`: channel half-bandwidth in Hz; `None` defaults to 5 000 Hz
    ///   (standard broadcast AM). Sets both the pre-filter anti-alias cutoff and the
    ///   initial audio IF filter cutoff.
    /// - `center_hz`: BFO offset from DC in Hz. `0.0` for DSB/envelope. For USB/LSB
    ///   this will shift the product detector carrier once that demod path is implemented.
    pub fn new(
        input_rate: u32,
        output_rate: u32,
        bandwidth_hz: Option<f32>,
        center_hz: f32,
    ) -> Self {
        let half_bw = bandwidth_hz.unwrap_or(5_000.0).max(2_000.0);

        let pre_decimation = ((input_rate / 50_000) as usize).max(1);
        let intermediate_rate = input_rate / pre_decimation as u32;

        let pre_filter = ComplexDecimatingFirFilter::new(
            pre_decimation,
            firdes::lowpass(half_bw / input_rate as f32, firdes::Kaiser::new(50.0)),
        );

        let audio_filter = FirFilter::new(Self::make_audio_taps_hz(half_bw, intermediate_rate));

        // ~30 Hz cutoff to remove carrier residual after envelope detect.
        let dc_alpha = 2.0 * std::f32::consts::PI * 30.0 / intermediate_rate as f32;

        Self {
            mode: AmMode::Envelope,
            center_hz,
            pre_filter,
            pre_decimation,
            intermediate_rate,
            dc_state: 0.0,
            dc_alpha,
            audio_filter,
            bandwidth_hz: half_bw,
            agc_gain: 1.0,
            agc_frozen: false,
            output_rate,
            rssi_db: -100.0,
            squelch_db: f32::NEG_INFINITY,
            squelch_hang_samples: 0,
            squelch_hang_remaining: 0,
            decimated_buf: Vec::new(),
            dc_blocked_buf: Vec::new(),
            filtered_buf: Vec::new(),
        }
    }

    pub fn set_mode(&mut self, mode: AmMode) {
        self.mode = mode;
    }

    pub fn freeze_agc(&mut self, frozen: bool) {
        self.agc_frozen = frozen;
    }

    /// Switch to a preset bandwidth step.
    pub fn set_bandwidth(&mut self, bw: AmBandwidth) {
        self.set_bandwidth_hz(bw.cutoff_hz());
    }

    /// Set the audio IF filter cutoff continuously in Hz.
    /// Use this for SSB where you want to dial in an exact passband width.
    pub fn set_bandwidth_hz(&mut self, hz: f32) {
        let hz = hz.max(2_000.0);
        if (hz - self.bandwidth_hz).abs() > 1.0 {
            self.audio_filter
                .set_taps(Self::make_audio_taps_hz(hz, self.intermediate_rate));
            self.bandwidth_hz = hz;
        }
    }

    /// Writes interleaved stereo PCM [L0, R0, L1, R1, …] into `out` (L == R, mono source).
    pub fn process_iq(&mut self, iq: &[Cf32], out: &mut Vec<f32>) {
        // RSSI — instantaneous in-band power in dBFS
        let power = iq.iter().map(|c| c.norm_sqr()).sum::<f32>() / iq.len() as f32;
        self.rssi_db = 10.0 * power.max(1e-10).log10();

        // Squelch gate
        if self.rssi_db < self.squelch_db {
            if self.squelch_hang_remaining == 0 {
                out.clear();
                return;
            }
        } else {
            self.squelch_hang_remaining = self.squelch_hang_samples;
        }

        // Stage 1: IQ anti-alias + decimate
        self.pre_filter.process_into(iq, &mut self.decimated_buf);

        if self.decimated_buf.is_empty() {
            out.clear();
            return;
        }

        // Stage 2: envelope detect + Stage 3: DC block (IIR, kept per-sample)
        self.dc_blocked_buf.clear();
        for i in 0..self.decimated_buf.len() {
            let s = self.decimated_buf[i];
            let demod = match self.mode {
                AmMode::Envelope => s.norm(),
                AmMode::Sam | AmMode::Usb | AmMode::Lsb | AmMode::Dsb => s.norm(),
            };
            self.dc_state += self.dc_alpha * (demod - self.dc_state);
            self.dc_blocked_buf.push(demod - self.dc_state);
        }

        // Stage 4: IF filter
        self.audio_filter.process_into(&self.dc_blocked_buf, &mut self.filtered_buf);

        // Stage 5: AGC + decimate to output rate + mono → interleaved stereo
        let audio_decimation = ((self.intermediate_rate / self.output_rate) as usize).max(1);
        let out_frames = self.filtered_buf.len() / audio_decimation;
        out.clear();
        out.reserve(out_frames * 2);

        for i in (0..self.filtered_buf.len()).step_by(audio_decimation) {
            let gained = self.filtered_buf[i] * self.agc_gain;
            if !self.agc_frozen {
                if gained.abs() > 0.8 {
                    self.agc_gain *= 0.999;
                } else {
                    self.agc_gain *= 1.0001;
                }
                self.agc_gain = self.agc_gain.clamp(0.001, 100.0);
            }
            let clipped = soft_clip(gained);
            out.push(clipped);
            out.push(clipped);
        }

        self.squelch_hang_remaining =
            self.squelch_hang_remaining.saturating_sub(out.len() / 2);
    }

    pub fn channel_half_bw_hz(&self) -> f32 {
        self.bandwidth_hz
    }

    pub fn rssi_db(&self) -> f32 {
        self.rssi_db
    }

    pub fn set_squelch(&mut self, threshold_db: f32, hang_ms: f32) {
        self.squelch_db = threshold_db;
        self.squelch_hang_samples =
            (hang_ms * self.output_rate as f32 / 1000.0).round() as usize;
    }

    fn make_audio_taps_hz(cutoff_hz: f32, rate: u32) -> Vec<f32> {
        firdes::lowpass(cutoff_hz / rate as f32, firdes::Kaiser::new(40.0))
    }
}

fn soft_clip(x: f32) -> f32 {
    x / (1.0 + x.abs())
}

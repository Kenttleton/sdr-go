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

    /// Returns interleaved stereo PCM [L0, R0, L1, R1, …] with L == R (mono source).
    pub fn process_iq(&mut self, iq: &[Cf32]) -> Vec<f32> {
        // Stage 1: IQ anti-alias + decimate
        let decimated = self.pre_filter.process(iq);

        if decimated.is_empty() {
            return Vec::new();
        }

        // Stage 2: envelope detect + Stage 3: DC block (IIR, kept per-sample)
        let mut dc_blocked = Vec::with_capacity(decimated.len());
        for &s in &decimated {
            let demod = match self.mode {
                AmMode::Envelope => s.norm(),
                AmMode::Sam | AmMode::Usb | AmMode::Lsb | AmMode::Dsb => s.norm(),
            };
            self.dc_state += self.dc_alpha * (demod - self.dc_state);
            dc_blocked.push(demod - self.dc_state);
        }

        // Stage 4: IF filter
        let filtered = self.audio_filter.process(&dc_blocked);

        // Stage 5: AGC + decimate to output rate + mono → interleaved stereo
        let audio_decimation = ((self.intermediate_rate / self.output_rate) as usize).max(1);
        let out_len = filtered.len() / audio_decimation;
        let mut out = Vec::with_capacity(out_len * 2);

        for &s in filtered.iter().step_by(audio_decimation) {
            let gained = s * self.agc_gain;
            if !self.agc_frozen {
                if gained.abs() > 0.8 {
                    self.agc_gain *= 0.999;
                } else {
                    self.agc_gain *= 1.0001;
                }
                self.agc_gain = self.agc_gain.clamp(0.001, 100.0);
            }
            out.push(gained);
            out.push(gained);
        }

        out
    }

    fn make_audio_taps_hz(cutoff_hz: f32, rate: u32) -> Vec<f32> {
        firdes::lowpass(cutoff_hz / rate as f32, firdes::Kaiser::new(40.0))
    }
}

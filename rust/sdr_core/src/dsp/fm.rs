use num_complex::Complex;
use super::filters::FirFilter;
use super::agc::Agc;

/// FM demodulation mode
#[derive(Debug, Clone, PartialEq)]
pub enum FmMode {
    Mono,
    Stereo,
    StereoFallbackMono,
}

/// Stereo FM pilot tone detector and decoder
/// FM stereo uses:
///   - 19kHz pilot tone to signal stereo presence
///   - 38kHz subcarrier (double pilot) carrying L-R difference signal
///   - Mono (L+R) sum signal in baseband 0-15kHz
struct StereoPilot {
    /// Phase accumulator for 19kHz pilot PLL
    phase: f32,
    /// Phase increment per sample at current sample rate
    phase_increment: f32,
    /// Detected pilot amplitude — used to confirm stereo signal present
    pilot_amplitude: f32,
    /// Low-pass filter for pilot amplitude detection
    amplitude_filter: f32,
}

impl StereoPilot {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            phase: 0.0,
            phase_increment: 2.0 * std::f32::consts::PI * 19_000.0
                / sample_rate as f32,
            pilot_amplitude: 0.0,
            amplitude_filter: 0.0,
        }
    }

    /// Returns true if stereo pilot is detected above threshold
    pub fn is_stereo(&self) -> bool {
        self.pilot_amplitude > 0.05
    }

    /// Process demodulated FM signal
    /// Returns 38kHz subcarrier reference for L-R decoding
    pub fn process(&mut self, sample: f32) -> f32 {
        // Generate 19kHz reference
        let pilot_ref = self.phase.cos();

        // Detect pilot amplitude via correlation
        let correlation = sample * pilot_ref;
        self.amplitude_filter = self.amplitude_filter * 0.9999
            + correlation.abs() * 0.0001;
        self.pilot_amplitude = self.amplitude_filter;

        // Advance phase
        self.phase += self.phase_increment;
        if self.phase > 2.0 * std::f32::consts::PI {
            self.phase -= 2.0 * std::f32::consts::PI;
        }

        // Return 38kHz reference (double the pilot phase)
        (self.phase * 2.0).cos()
    }
}

/// De-emphasis filter — 75µs for North America, 50µs for Europe
struct DeEmphasis {
    alpha: f32,
    prev: f32,
}

impl DeEmphasis {
    /// 75µs — North America and South Korea
    pub fn us75(sample_rate: f32) -> Self {
        let tau = 75e-6;
        let alpha = 1.0 - (-1.0 / (tau * sample_rate)).exp();
        Self { alpha, prev: 0.0 }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        self.prev = self.prev + self.alpha * (sample - self.prev);
        self.prev
    }
}

/// Output from FM demodulator — either mono or stereo frame
pub enum FmAudioFrame {
    Mono(Vec<f32>),
    Stereo(Vec<f32>, Vec<f32>), // (left, right)
}

/// Wideband FM demodulator with optional stereo decoding
pub struct FmDemodulator {
    prev: Complex<f32>,
    pre_filter: FirFilter,
    /// Separate audio filters per channel for stereo
    audio_filter_sum: FirFilter,   // L+R
    audio_filter_diff: FirFilter,  // L-R
    deemphasis_l: DeEmphasis,
    deemphasis_r: DeEmphasis,
    pilot: StereoPilot,
    /// Bandpass filter centered on 38kHz difference subcarrier
    diff_filter: FirFilter,
    mode: FmMode,
    decimation: usize,
    decimate_count: usize,
    agc: Agc,
    /// Intermediate sample rate after FM discriminator, before audio decimation
    /// This is where pilot detection and stereo decoding happen
    intermediate_rate: u32,
}

impl FmDemodulator {
    pub fn new(input_rate: u32, output_rate: u32, mode: FmMode) -> Self {
        // FM discriminator runs at input_rate
        // We need enough bandwidth for the stereo difference signal at 38kHz
        // So we decimate in two stages:
        //   Stage 1: input_rate → intermediate_rate (keeps stereo subcarrier)
        //   Stage 2: intermediate_rate → output_rate (audio decimation)
        //
        // Intermediate rate must be > 2 * 53kHz = 106kHz for stereo
        // 192kHz is a clean choice that works for all output rates

        let intermediate_rate = 192_000u32;
        let stage1_decimation = (input_rate / intermediate_rate).max(1) as usize;
        let stage2_decimation = (intermediate_rate / output_rate).max(1) as usize;
        let decimation = stage1_decimation * stage2_decimation;

        // Pre-filter: pass full FM multiplex signal (0-53kHz)
        let pre_cutoff = 53_000.0 / input_rate as f32;
        let pre_filter = FirFilter::low_pass(pre_cutoff, 128);

        // Sum channel filter: 0-15kHz (mono/L+R content)
        let sum_cutoff = 15_000.0 / intermediate_rate as f32;
        let audio_filter_sum = FirFilter::low_pass(sum_cutoff, 64);

        // Difference channel filter: bandpass around 38kHz subcarrier
        // Approximate with low-pass then we'll mix down
        let diff_cutoff = 15_000.0 / intermediate_rate as f32;
        let audio_filter_diff = FirFilter::low_pass(diff_cutoff, 64);
        let diff_filter = FirFilter::low_pass(53_000.0 / intermediate_rate as f32, 64);

        Self {
            prev: Complex::new(1.0, 0.0),
            pre_filter,
            audio_filter_sum,
            audio_filter_diff,
            deemphasis_l: DeEmphasis::us75(output_rate as f32),
            deemphasis_r: DeEmphasis::us75(output_rate as f32),
            pilot: StereoPilot::new(intermediate_rate),
            diff_filter,
            mode,
            decimation,
            decimate_count: 0,
            agc: Agc::default_fm(),
            intermediate_rate,
        }
    }

    pub fn process(&mut self, samples: &[Complex<f32>]) -> FmAudioFrame {
        let capacity = samples.len() / self.decimation;
        let mut left = Vec::with_capacity(capacity);
        let mut right = Vec::with_capacity(capacity);

        for &sample in samples {
            // FM phase discriminator
            let product = sample * self.prev.conj();
            let demodulated = product.im.atan2(product.re);
            self.prev = sample;

            self.decimate_count += 1;
            if self.decimate_count < self.decimation {
                continue;
            }
            self.decimate_count = 0;

            // Get 38kHz subcarrier reference from pilot PLL
            let subcarrier_ref = self.pilot.process(demodulated);
            let is_stereo = self.pilot.is_stereo();

            // Sum signal (L+R) — low-pass filtered baseband
            let sum = self.audio_filter_sum.process(demodulated);

            let (l, r) = match (&self.mode, is_stereo) {
                (FmMode::Mono, _)
                | (FmMode::StereoFallbackMono, false) => {
                    // Mono path — apply de-emphasis and AGC once
                    let deemphasized = self.deemphasis_l.process(sum);
                    let gained = self.agc.process(deemphasized);
                    (gained, gained)
                }
                (FmMode::Stereo, _)
                | (FmMode::StereoFallbackMono, true) => {
                    // Stereo path
                    // Mix demodulated signal with 38kHz reference to recover L-R
                    let mixed = demodulated * subcarrier_ref;
                    let diff = self.audio_filter_diff.process(mixed);

                    // Matrix decode: L = (L+R) + (L-R), R = (L+R) - (L-R)
                    let l_raw = sum + diff;
                    let r_raw = sum - diff;

                    // Independent de-emphasis per channel
                    let l_de = self.deemphasis_l.process(l_raw);
                    let r_de = self.deemphasis_r.process(r_raw);

                    // AGC on left channel, apply same gain to right
                    // to preserve stereo image
                    let gained_l = self.agc.process(l_de);
                    let gained_r = r_de * self.agc.gain;

                    (gained_l, gained_r)
                }
            };

            left.push(l);
            right.push(r);
        }

        // Return appropriate frame type
        match &self.mode {
            FmMode::Mono => FmAudioFrame::Mono(left),
            FmMode::Stereo | FmMode::StereoFallbackMono => {
                if self.pilot.is_stereo() {
                    FmAudioFrame::Stereo(left, right)
                } else {
                    FmAudioFrame::Mono(left)
                }
            }
        }
    }

    /// Whether stereo pilot is currently detected
    pub fn is_stereo_detected(&self) -> bool {
        self.pilot.is_stereo()
    }
}
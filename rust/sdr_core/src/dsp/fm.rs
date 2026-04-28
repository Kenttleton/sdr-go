use num_complex::Complex;
use super::filters::FirFilter;
use super::agc::Agc;

/// Wideband FM demodulator for broadcast FM (87.5–108 MHz)
/// Also usable for NOAA weather audio with narrower filter
pub struct FmDemodulator {
    /// Previous IQ sample for phase discriminator
    prev: Complex<f32>,
    /// Low-pass filter applied to IQ before demodulation
    pre_filter: FirFilter,
    /// Audio low-pass filter after demodulation
    audio_filter: FirFilter,
    /// De-emphasis filter (75µs for North America, 50µs for Europe)
    deemphasis: DeEmphasis,
    /// Decimation factor — reduces sample rate to audio rate
    decimation: usize,
    /// Decimation counter
    decimate_count: usize,
    /// AGC on output audio
    agc: Agc,
}

/// RC de-emphasis filter
/// FM broadcast pre-emphasizes high frequencies at the transmitter
/// We must reverse this at the receiver
struct DeEmphasis {
    alpha: f32,
    prev: f32,
}

impl DeEmphasis {
    /// 75µs time constant — standard for North America and South Korea
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

impl FmDemodulator {
    /// Standard wideband FM broadcast demodulator
    /// input_rate: IQ sample rate from RTL-SDR (e.g. 2_048_000)
    /// output_rate: desired audio sample rate (e.g. 48_000)
    pub fn new(input_rate: u32, output_rate: u32) -> Self {
        let decimation = (input_rate / output_rate) as usize;

        // Pre-filter: pass FM broadcast bandwidth (~200 kHz), reject adjacent
        // Normalized cutoff = 100kHz / 2_048_000 ≈ 0.049
        let cutoff = 100_000.0 / input_rate as f32;
        let pre_filter = FirFilter::low_pass(cutoff, 64);

        // Audio filter: pass audio bandwidth (15 kHz max for FM)
        let audio_cutoff = 15_000.0 / output_rate as f32;
        let audio_filter = FirFilter::low_pass(audio_cutoff, 32);

        Self {
            prev: Complex::new(1.0, 0.0),
            pre_filter,
            audio_filter,
            deemphasis: DeEmphasis::us75(output_rate as f32),
            decimation,
            decimate_count: 0,
            agc: Agc::default_fm(),
        }
    }

    /// Process a block of IQ samples
    /// Returns decimated audio samples ready for playback
    pub fn process(&mut self, samples: &[Complex<f32>]) -> Vec<f32> {
        let mut audio = Vec::with_capacity(samples.len() / self.decimation);

        for &sample in samples {
            // Phase discriminator — FM demodulation
            // Computes instantaneous frequency as phase difference
            let product = sample * self.prev.conj();
            let demodulated = product.im.atan2(product.re);
            self.prev = sample;

            // Decimate to audio rate
            self.decimate_count += 1;
            if self.decimate_count >= self.decimation {
                self.decimate_count = 0;

                // De-emphasis
                let deemphasized = self.deemphasis.process(demodulated);

                // Audio low-pass filter
                let filtered = self.audio_filter.process(deemphasized);

                // AGC
                let gained = self.agc.process(filtered);

                audio.push(gained);
            }
        }

        audio
    }
}
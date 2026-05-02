use super::agc::Agc;
use super::filters::FIRFilter;
use num_complex::Complex;

#[derive(Debug, Clone, PartialEq)]
pub enum FMDemodulationMode {
    Narrow,
    Wide,
}

pub enum FMAudioFrame {
    Mono(Vec<f32>),
    Stereo(Vec<f32>, Vec<f32>),
    //Data(Vec<f32>),
}

pub struct FMDemodulator {
    prev: Complex<f32>,
    channel_filter_i: FIRFilter,
    channel_filter_q: FIRFilter,
    audio_filter: FIRFilter,
    agc: Agc,
    dec_factor: usize,
    dec_count: usize,
    pub signal_power: f32,
    mode: FMDemodulationMode,
}

impl FMDemodulator {
    pub fn new(input_rate: u32, audio_rate: u32, mode: FMDemodulationMode) -> Self {
        let dec_factor = (input_rate / audio_rate).max(1) as usize;

        let ch_cutoff = match mode {
            Narrow => 6_250.0 / input_rate as f32,
            Wide => 100_000.0 / input_rate as f32,
            _ => 100_000.0 / input_rate as f32,
        };

        // Audio LPF: 15 kHz cutoff normalised to input_rate. 128 taps gives a
        // transition band of input_rate/128 ≈ 9.4 kHz at 1.2 MHz, putting the
        // pilot (19 kHz) well into the stopband.
        let audio_cutoff = 15_000.0 / input_rate as f32;

        Self {
            prev: Complex::new(1.0, 0.0),
            channel_filter_i: FIRFilter::low_pass(ch_cutoff, 64),
            channel_filter_q: FIRFilter::low_pass(ch_cutoff, 64),
            audio_filter: FIRFilter::low_pass(audio_cutoff, 128),
            agc: Agc::default_fm(),
            dec_factor,
            dec_count: 0,
            signal_power: 0.0,
            mode,
        }
    }

    /// Stations mode — same pipeline, different rates.
    pub fn new_stations(input_rate: u32, audio_rate: u32, mode: FMDemodulationMode) -> Self {
        Self::new(input_rate, audio_rate, mode)
    }

    pub fn process(&mut self, samples: &[Complex<f32>]) -> FMAudioFrame {
        let mut out = Vec::with_capacity(samples.len() / self.dec_factor);

        for &raw in samples {
            // IQ channel filter — both I and Q must have independent state.
            let fi = self.channel_filter_i.process(raw.re);
            let fq = self.channel_filter_q.process(raw.im);
            let filtered = Complex::new(fi, fq);

            // FM discriminator at input_rate.
            // Phase differences are small at high rates (≈0.39 rad at 1.2 MHz
            // for 75 kHz deviation), so atan2 is always accurate.
            let product = filtered * self.prev.conj();
            let demod = product.im.atan2(product.re);
            self.prev = filtered;

            // Signal power tracked at input_rate for the strength indicator.
            let power = raw.norm_sqr();
            self.signal_power = self.signal_power * 0.9999 + power * 0.0001;

            // Audio LPF also runs at input_rate (anti-alias before decimation).
            let audio = self.audio_filter.process(demod);

            self.dec_count += 1;
            if self.dec_count < self.dec_factor {
                continue;
            }
            self.dec_count = 0;

            // Scale from rad/sample-at-input-rate → rad/sample-at-audio-rate.
            // FM phase differences add linearly, so this is exact.
            let scaled = audio * self.dec_factor as f32;
            let gained = self.agc.process(scaled);
            out.push(gained);
        }

        FMAudioFrame::Mono(out)
    }

    // ── Runtime controls (stubs until stereo/EQ are re-added) ─────────────────

    pub fn set_mode(&mut self, _mode: FMDemodulationMode) {}

    pub fn set_eq(&mut self, _gains: &[f32; 7]) {}

    pub fn is_stereo_detected(&self) -> bool {
        false
    }

    pub fn pilot_amplitude(&self) -> f32 {
        0.0
    }

    pub fn rds(&self) -> Option<&super::rds::RdsDecoder> {
        None
    }
}

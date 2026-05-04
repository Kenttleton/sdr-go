mod am;
mod ddc;
mod filters;
mod fm;
mod manager;
pub mod spectrum;

use am::AmPipeline;
use fm::{FmMode, FmPipeline};
use num_complex::Complex;

pub use am::{AmBandwidth, AmMode};
pub use manager::{PipelineManager, PipelineMode};
pub use spectrum::{FftStage, WaveformStage};

type Cf32 = Complex<f32>;

/// Supported demodulation pipelines.
/// Sub-mode changes within a variant are dynamic (no transition).
/// Crossing between variants (Fm ↔ Am) goes through PipelineManager::switch_mode.
pub enum DemodPipeline {
    Fm(FmPipeline),
    Am(AmPipeline),
}

impl DemodPipeline {
    /// Wide FM — broadcast band, stereo pilot tracking, 75 µs de-emphasis.
    pub fn wfm(input_rate: u32, output_rate: u32, stereo: bool) -> Self {
        Self::Fm(FmPipeline::new(input_rate, output_rate, stereo, FmMode::Wide, None, 0.0))
    }

    /// Narrow FM — voice/utility bands, mono, no de-emphasis.
    /// `bandwidth_hz`: channel half-bandwidth override (default 12.5 kHz).
    pub fn nfm(input_rate: u32, output_rate: u32, bandwidth_hz: Option<f32>) -> Self {
        Self::Fm(FmPipeline::new(input_rate, output_rate, false, FmMode::Narrow, bandwidth_hz, 0.0))
    }

    /// AM double sideband / envelope detect.
    /// `bandwidth_hz`: channel half-bandwidth (default 5 000 Hz).
    pub fn am_dsb(input_rate: u32, output_rate: u32, bandwidth_hz: Option<f32>) -> Self {
        Self::Am(AmPipeline::new(input_rate, output_rate, bandwidth_hz, 0.0))
    }

    /// AM upper sideband. `center_hz` is the BFO offset above the carrier (stub).
    pub fn am_usb(input_rate: u32, output_rate: u32, bandwidth_hz: Option<f32>, center_hz: f32) -> Self {
        let mut p = AmPipeline::new(input_rate, output_rate, bandwidth_hz, center_hz);
        p.set_mode(AmMode::Usb);
        Self::Am(p)
    }

    /// AM lower sideband. `center_hz` is the BFO offset below the carrier (stub).
    pub fn am_lsb(input_rate: u32, output_rate: u32, bandwidth_hz: Option<f32>, center_hz: f32) -> Self {
        let mut p = AmPipeline::new(input_rate, output_rate, bandwidth_hz, center_hz);
        p.set_mode(AmMode::Lsb);
        Self::Am(p)
    }

    pub fn process_iq(&mut self, iq: &[Cf32]) -> Vec<f32> {
        match self {
            Self::Fm(p) => p.process_iq(iq),
            Self::Am(p) => p.process_iq(iq),
        }
    }

    // ── FM dynamic controls ───────────────────────────────────────────────────

    /// Enable or disable FM stereo decode.
    /// Returns false if stereo was requested but no pilot tone is detected.
    pub fn set_fm_stereo(&mut self, enabled: bool) -> bool {
        match self {
            Self::Fm(p) => p.set_stereo(enabled),
            _ => false,
        }
    }

    // ── AM dynamic controls ───────────────────────────────────────────────────

    /// Change AM demod sub-mode without rebuilding the pipeline.
    pub fn set_am_mode(&mut self, mode: AmMode) {
        if let Self::Am(p) = self {
            p.set_mode(mode);
        }
    }

    /// Replace the AM IF filter using a preset step.
    pub fn set_am_bandwidth(&mut self, bw: AmBandwidth) {
        if let Self::Am(p) = self {
            p.set_bandwidth(bw);
        }
    }

    /// Replace the AM IF filter with a continuous Hz cutoff.
    /// More precise than the preset steps — use this for SSB passband dialing.
    pub fn set_am_bandwidth_hz(&mut self, hz: f32) {
        if let Self::Am(p) = self {
            p.set_bandwidth_hz(hz);
        }
    }

    /// Freeze or unfreeze AGC. Frozen during output crossfade so gain doesn't
    /// surge on the incoming demod while its signal level is ramping up.
    pub fn freeze_agc(&mut self, frozen: bool) {
        match self {
            Self::Fm(p) => p.freeze_agc(frozen),
            Self::Am(p) => p.freeze_agc(frozen),
        }
    }

    // ── Shared queries ────────────────────────────────────────────────────────

    pub fn is_stereo_detected(&self) -> bool {
        match self {
            Self::Fm(p) => p.is_stereo_detected(),
            Self::Am(_) => false,
        }
    }
}

mod am;
mod ddc;
mod filters;
mod fm;
mod manager;
pub mod spectrum;

use am::AmPipeline;
use fm::FmPipeline;
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
    pub fn fm(input_rate: u32, output_rate: u32, stereo: bool) -> Self {
        Self::Fm(FmPipeline::new(input_rate, output_rate, stereo))
    }

    pub fn am(input_rate: u32, output_rate: u32) -> Self {
        Self::Am(AmPipeline::new(input_rate, output_rate))
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

    /// Replace the AM IF filter for the new bandwidth setting.
    pub fn set_am_bandwidth(&mut self, bw: AmBandwidth) {
        if let Self::Am(p) = self {
            p.set_bandwidth(bw);
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

use num_complex::Complex;

use super::{ddc::Ddc, AmBandwidth, AmMode, DemodPipeline};

type Cf32 = Complex<f32>;

/// Top-level pipeline mode exposed to JNI callers.
pub enum PipelineMode {
    Fm,
    Am,
}

enum PipelineState {
    Stable,
    /// Hardware just retuned; discard this many output samples while PLL/AGC settle.
    Retuning { samples_remaining: usize },
    /// Two demodulators running simultaneously; outputs blended over `crossfade_len`
    /// PCM elements (~1024 stereo frames).  `self.demod` is the incoming pipeline.
    CrossfadingMode {
        outgoing: DemodPipeline,
        progress: usize,
        crossfade_len: usize,
    },
}

pub struct PipelineManager {
    demod: DemodPipeline,
    state: PipelineState,
    input_rate: u32,
    output_rate: u32,
    stereo: bool,
    ddc: Ddc,
    center_hz: u32,
}

impl PipelineManager {
    pub fn new(input_rate: u32, output_rate: u32, stereo: bool, center_hz: u32) -> Self {
        Self {
            demod: DemodPipeline::fm(input_rate, output_rate, stereo),
            state: PipelineState::Stable,
            input_rate,
            output_rate,
            stereo,
            ddc: Ddc::new(input_rate as f32),
            center_hz,
        }
    }

    // ── Frequency control ─────────────────────────────────────────────────────

    pub fn center_hz(&self) -> u32 {
        self.center_hz
    }

    /// Apply a small frequency offset digitally without touching hardware.
    /// Call when the requested frequency is within ±1 MHz of `center_hz`.
    pub fn set_ddc_offset(&mut self, offset_hz: f32) {
        self.ddc.set_offset(offset_hz);
    }

    /// Called after a hardware retune completes.
    /// Clears the DDC offset (hardware is now at the target frequency) and
    /// begins discarding samples while the PLL and AGC re-settle.
    pub fn note_hardware_retune(&mut self, new_center_hz: u32) {
        self.center_hz = new_center_hz;
        self.ddc.set_offset(0.0);
        // If a crossfade was in progress, consider it complete so the incoming
        // demod's AGC is not left frozen through the retune silence.
        if matches!(self.state, PipelineState::CrossfadingMode { .. }) {
            self.demod.freeze_agc(false);
        }
        self.state = PipelineState::Retuning {
            samples_remaining: 204_800,
        };
    }

    // ── Mode switching ────────────────────────────────────────────────────────

    /// Begin a smooth mode transition.  Both demodulators run on every IQ block
    /// during the crossfade; their outputs are linearly blended over 2048 PCM
    /// elements (~1024 stereo frames at 48 kHz ≈ 21 ms).
    pub fn switch_mode(&mut self, mode: PipelineMode) {
        // Drop any in-progress transition cleanly before starting a new one.
        self.state = PipelineState::Stable;

        let incoming = self.build_demod(mode);
        let outgoing = std::mem::replace(&mut self.demod, incoming);
        // Freeze the incoming demod's AGC so it tracks from a stable gain once
        // the crossfade completes rather than chasing a ramping signal.
        self.demod.freeze_agc(true);

        self.state = PipelineState::CrossfadingMode {
            outgoing,
            progress: 0,
            crossfade_len: 2048,
        };
    }

    // ── FM dynamic controls ───────────────────────────────────────────────────

    pub fn set_fm_stereo(&mut self, enabled: bool) -> bool {
        self.stereo = enabled;
        self.demod.set_fm_stereo(enabled)
    }

    // ── AM dynamic controls ───────────────────────────────────────────────────

    pub fn set_am_mode(&mut self, mode: AmMode) {
        self.demod.set_am_mode(mode);
    }

    pub fn set_am_bandwidth(&mut self, bw: AmBandwidth) {
        self.demod.set_am_bandwidth(bw);
    }

    // ── Processing ───────────────────────────────────────────────────────────

    /// Process a block of IQ samples.
    ///
    /// Takes ownership so DDC can be applied in-place.  The waveform snapshot
    /// in `lib.rs` is captured before this call and always reflects raw RF.
    pub fn process_iq(&mut self, mut iq: Vec<Cf32>) -> Vec<f32> {
        self.ddc.process(&mut iq);

        // Replace state with Stable so we can move out of it; re-set below.
        match std::mem::replace(&mut self.state, PipelineState::Stable) {
            PipelineState::Stable => {
                self.state = PipelineState::Stable;
                self.demod.process_iq(&iq)
            }

            PipelineState::Retuning { mut samples_remaining } => {
                let to_discard = samples_remaining.min(iq.len());
                samples_remaining -= to_discard;
                self.state = if samples_remaining > 0 {
                    PipelineState::Retuning { samples_remaining }
                } else {
                    PipelineState::Stable
                };
                Vec::new()
            }

            PipelineState::CrossfadingMode {
                mut outgoing,
                mut progress,
                crossfade_len,
            } => {
                let out_pcm = outgoing.process_iq(&iq);
                let in_pcm = self.demod.process_iq(&iq);

                let len = out_pcm.len().min(in_pcm.len());
                let mut blended = Vec::with_capacity(len);
                for i in 0..len {
                    let alpha = (progress as f32 / crossfade_len as f32).min(1.0);
                    blended.push(out_pcm[i] * (1.0 - alpha) + in_pcm[i] * alpha);
                    progress += 1;
                }

                if progress >= crossfade_len {
                    self.demod.freeze_agc(false);
                    self.state = PipelineState::Stable;
                } else {
                    self.state = PipelineState::CrossfadingMode {
                        outgoing,
                        progress,
                        crossfade_len,
                    };
                }

                blended
            }
        }
    }

    pub fn is_stereo_detected(&self) -> bool {
        self.demod.is_stereo_detected()
    }

    fn build_demod(&self, mode: PipelineMode) -> DemodPipeline {
        match mode {
            PipelineMode::Fm => DemodPipeline::fm(self.input_rate, self.output_rate, self.stereo),
            PipelineMode::Am => DemodPipeline::am(self.input_rate, self.output_rate),
        }
    }
}

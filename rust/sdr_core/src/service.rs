use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

use num_complex::Complex;

use crate::pipeline::{AmBandwidth, AmMode, PipelineManager, PipelineMode};

type Cf32 = Complex<f32>;

// ── Commands ──────────────────────────────────────────────────────────────────

pub enum Command {
    /// Adjust receive frequency via DDC offset from the current hardware center.
    /// Large jumps (> ±1 MHz) should be preceded by a hardware retune at the
    /// device layer, followed by NoteHardwareRetune.
    SetDdcOffset(f32),
    /// Notify the pipeline that the hardware has retuned to a new center.
    NoteHardwareRetune(u32),
    SetMode(PipelineMode),
    SetFmStereo(bool),
    SetAmMode(AmMode),
    SetAmBandwidthHz(f32),
    SetAmBandwidth(AmBandwidth),
    SetSquelch { threshold_db: f32, hang_ms: f32 },
}

// ── Metadata ──────────────────────────────────────────────────────────────────

pub struct Metadata {
    pub rssi_db: f32,
    pub stereo_detected: bool,
    pub center_hz: u32,
}

// ── Service ───────────────────────────────────────────────────────────────────

/// Owns the DSP pipeline and runs on the audio thread.
/// Call `process` once per IQ block; commands from `RadioServiceHandle` are
/// applied non-blocking at the top of each block.
pub struct RadioService {
    pipeline: PipelineManager,
    cmd_rx: Receiver<Command>,
    pcm_buf: Vec<f32>,
}

/// Cloneable handle for the control / WebSocket thread.
/// All sends are non-blocking from the caller's perspective; the DSP thread
/// drains the channel between blocks with no mutex involved.
#[derive(Clone)]
pub struct RadioServiceHandle {
    cmd_tx: Sender<Command>,
}

impl RadioService {
    pub fn new(
        input_rate: u32,
        output_rate: u32,
        stereo: bool,
        center_hz: u32,
    ) -> (Self, RadioServiceHandle) {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let service = Self {
            pipeline: PipelineManager::new(input_rate, output_rate, stereo, center_hz),
            cmd_rx,
            pcm_buf: Vec::new(),
        };
        (service, RadioServiceHandle { cmd_tx })
    }

    /// Process one IQ block.  Drains pending commands first (lock-free), then
    /// runs DSP.  Returns the interleaved stereo PCM output.
    pub fn process(&mut self, iq: Vec<Cf32>) -> &[f32] {
        loop {
            match self.cmd_rx.try_recv() {
                Ok(cmd) => self.apply(cmd),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }

        self.pipeline.process_iq(iq, &mut self.pcm_buf);
        &self.pcm_buf
    }

    pub fn metadata(&self) -> Metadata {
        Metadata {
            rssi_db: self.pipeline.rssi_db(),
            stereo_detected: self.pipeline.is_stereo_detected(),
            center_hz: self.pipeline.center_hz(),
        }
    }

    fn apply(&mut self, cmd: Command) {
        match cmd {
            Command::SetDdcOffset(hz) => self.pipeline.set_ddc_offset(hz),
            Command::NoteHardwareRetune(center_hz) => {
                self.pipeline.note_hardware_retune(center_hz)
            }
            Command::SetMode(mode) => self.pipeline.switch_mode(mode),
            Command::SetFmStereo(enabled) => {
                self.pipeline.set_fm_stereo(enabled);
            }
            Command::SetAmMode(mode) => self.pipeline.set_am_mode(mode),
            Command::SetAmBandwidthHz(hz) => self.pipeline.set_am_bandwidth_hz(hz),
            Command::SetAmBandwidth(bw) => self.pipeline.set_am_bandwidth(bw),
            Command::SetSquelch { threshold_db, hang_ms } => {
                self.pipeline.set_squelch(threshold_db, hang_ms)
            }
        }
    }
}

impl RadioServiceHandle {
    pub fn set_ddc_offset(&self, hz: f32) -> Result<(), mpsc::SendError<Command>> {
        self.cmd_tx.send(Command::SetDdcOffset(hz))
    }

    pub fn note_hardware_retune(&self, center_hz: u32) -> Result<(), mpsc::SendError<Command>> {
        self.cmd_tx.send(Command::NoteHardwareRetune(center_hz))
    }

    pub fn set_mode(&self, mode: PipelineMode) -> Result<(), mpsc::SendError<Command>> {
        self.cmd_tx.send(Command::SetMode(mode))
    }

    pub fn set_fm_stereo(&self, enabled: bool) -> Result<(), mpsc::SendError<Command>> {
        self.cmd_tx.send(Command::SetFmStereo(enabled))
    }

    pub fn set_am_mode(&self, mode: AmMode) -> Result<(), mpsc::SendError<Command>> {
        self.cmd_tx.send(Command::SetAmMode(mode))
    }

    pub fn set_am_bandwidth_hz(&self, hz: f32) -> Result<(), mpsc::SendError<Command>> {
        self.cmd_tx.send(Command::SetAmBandwidthHz(hz))
    }

    pub fn set_am_bandwidth(&self, bw: AmBandwidth) -> Result<(), mpsc::SendError<Command>> {
        self.cmd_tx.send(Command::SetAmBandwidth(bw))
    }

    pub fn set_squelch(
        &self,
        threshold_db: f32,
        hang_ms: f32,
    ) -> Result<(), mpsc::SendError<Command>> {
        self.cmd_tx.send(Command::SetSquelch { threshold_db, hang_ms })
    }
}

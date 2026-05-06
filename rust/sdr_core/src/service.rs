use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

use num_complex::Complex;

use crate::pipeline::{AmBandwidth, AmMode, PipelineManager, PipelineMode};
use crate::usb::IqStream;

type Cf32 = Complex<f32>;

// ── Commands ──────────────────────────────────────────────────────────────────

pub enum Command {
    /// Tune to a channel frequency. The service decides internally whether a DDC
    /// shift suffices or a hardware retune is required based on sample bandwidth
    /// and channel bandwidth.
    SetChannelFrequency(u32),
    /// Adjust receive frequency via DDC offset from the current hardware center.
    SetDdcOffset(f32),
    /// Notify the pipeline that the hardware has retuned to a new center.
    /// Use SetChannelFrequency instead when the service owns the IqStream.
    NoteHardwareRetune(u32),
    SetMode(PipelineMode),
    SetFmStereo(bool),
    SetAmMode(AmMode),
    SetAmBandwidthHz(f32),
    SetAmBandwidth(AmBandwidth),
    SetSquelch { threshold_db: f32, hang_ms: f32 },
}

// ── Metadata ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct Metadata {
    pub rssi_db: f32,
    pub stereo_detected: bool,
    pub center_hz: u32,
}

// ── Service ───────────────────────────────────────────────────────────────────

/// Owns the DSP pipeline and runs on the audio thread.
/// Call `tick` each iteration when the service owns an IqStream (sdr_srv use).
/// Call `process(iq)` to inject IQ externally (JNI use).
/// Commands from `RadioServiceHandle` are applied non-blocking at the top of each block.
pub struct RadioService {
    pipeline: PipelineManager,
    cmd_rx: Receiver<Command>,
    pcm_buf: Vec<f32>,
    stream: Option<IqStream>,
    stub_rng: u32,
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
        stream: Option<IqStream>,
    ) -> (Self, RadioServiceHandle) {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let service = Self {
            pipeline: PipelineManager::new(input_rate, output_rate, stereo, center_hz),
            cmd_rx,
            pcm_buf: Vec::new(),
            stream,
            stub_rng: 0xdeadbeef,
        };
        (service, RadioServiceHandle { cmd_tx })
    }

    /// Drive one iteration from the internal IqStream (or stub when no stream is set).
    /// Use this in the sdr_srv DSP thread.
    pub fn tick(&mut self) -> &[f32] {
        self.drain_commands();
        let iq = self.read_iq();
        self.pipeline.process_iq(iq, &mut self.pcm_buf);
        &self.pcm_buf
    }

    /// Process one externally-supplied IQ block (JNI / Android use).
    /// Drains pending commands first (lock-free).
    pub fn process(&mut self, iq: Vec<Cf32>) -> &[f32] {
        self.drain_commands();
        self.pipeline.process_iq(iq, &mut self.pcm_buf);
        &self.pcm_buf
    }

    fn drain_commands(&mut self) {
        loop {
            match self.cmd_rx.try_recv() {
                Ok(cmd) => self.apply(cmd),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
    }

    fn read_iq(&mut self) -> Vec<Cf32> {
        if let Some(ref mut s) = self.stream {
            let _ = s.fill();
            let avail = s.available();
            if avail > 0 {
                return s.drain(avail);
            }
        }
        // Stub: white noise for testing without hardware.
        let rng = &mut self.stub_rng;
        (0..8_192).map(|_| {
            *rng ^= *rng << 13;
            *rng ^= *rng >> 17;
            *rng ^= *rng << 5;
            Cf32::new(
                (*rng as i32 as f32) / (i32::MAX as f32) * 0.1,
                ((*rng).wrapping_mul(1664525).wrapping_add(1013904223) as i32 as f32)
                    / (i32::MAX as f32)
                    * 0.1,
            )
        }).collect()
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
            Command::SetChannelFrequency(target_hz) => {
                if self.pipeline.requires_hardware_retune(target_hz) {
                    let result = self.stream.as_mut().map(|s| s.retune(target_hz));
                    match result {
                        Some(Ok(())) => {}
                        Some(Err(e)) => log::warn!("hardware retune to {} Hz failed: {}", target_hz, e),
                        None => log::info!("no hardware stream; updating pipeline center to {} Hz (stub)", target_hz),
                    }
                    self.pipeline.note_hardware_retune(target_hz);
                } else {
                    let offset = target_hz as i64 - self.pipeline.center_hz() as i64;
                    self.pipeline.set_ddc_offset(offset as f32);
                    log::debug!("DDC offset → {} Hz (channel {} Hz)", offset, target_hz);
                }
            }
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
    /// Primary tune method. The service decides DDC vs hardware retune internally.
    pub fn set_channel_frequency(&self, hz: u32) -> Result<(), mpsc::SendError<Command>> {
        self.cmd_tx.send(Command::SetChannelFrequency(hz))
    }

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

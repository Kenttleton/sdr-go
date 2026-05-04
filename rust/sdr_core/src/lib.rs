//! sdr_core – JNI entry point and pipeline orchestration.

#![allow(dead_code)]
#![allow(unused_imports)]

mod pipeline;
mod usb;

use once_cell::sync::Lazy;
use parking_lot::Mutex;

use jni::objects::JClass;
use jni::sys::{jboolean, jfloat, jfloatArray, jint, jlong};
use jni::JNIEnv;

use num_complex::Complex;

use pipeline::{FftStage, PipelineManager, PipelineMode, WaveformStage};
use usb::{DeviceConfig, IqStream, SdrDevice};

type Cf32 = Complex<f32>;

// ── Global pipeline state ─────────────────────────────────────────────────────

struct Pipeline {
    device: SdrDevice,
    stream: IqStream,
    manager: PipelineManager,
    fft: FftStage,
    waveform: WaveformStage,
}

unsafe impl Send for Pipeline {}

static PIPELINE: Lazy<Mutex<Option<Pipeline>>> = Lazy::new(|| Mutex::new(None));

// ═════════════════════════════════════════════════════════════════════════════
// JNI exports
// ═════════════════════════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_coreVersion(
    env: JNIEnv,
    _class: JClass,
) -> jni::sys::jstring {
    let version = format!(
        "sdr_core v{} · futuredsp · rustfft",
        env!("CARGO_PKG_VERSION"),
    );
    env.new_string(version)
        .expect("Failed to create string")
        .into_raw()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_openDevice(
    mut env: JNIEnv,
    _class: JClass,
    fd: jint,
    frequency_hz: jlong,
    audio_sample_rate: jint,
    stereo: jboolean,
) -> jboolean {
    let config = DeviceConfig {
        frequency_hz: frequency_hz as u32,
        audio_sample_rate: audio_sample_rate as u32,
        ..Default::default()
    };

    match SdrDevice::open_from_fd(fd, config.clone()) {
        Ok(device) => {
            let stream = IqStream::new(device.inner(), device.bulk_transfer_samples(), None);
            let manager = PipelineManager::new(
                config.sample_rate,
                config.audio_sample_rate,
                stereo != 0,
                config.frequency_hz,
            );

            *PIPELINE.lock() = Some(Pipeline {
                device,
                stream,
                manager,
                fft: FftStage::new(2048),
                waveform: WaveformStage::new(),
            });
            1
        }
        Err(e) => {
            let _ = env.throw_new(
                "java/lang/RuntimeException",
                format!("Failed to open SDR device: {e}"),
            );
            0
        }
    }
}

// Returns: 0 = error, 1 = DDC software tune, 2 = hardware retune (settling started).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_setFrequency(
    mut env: JNIEnv,
    _class: JClass,
    frequency_hz: jlong,
) -> jint {
    let mut lock = PIPELINE.lock();
    let p = match lock.as_mut() {
        Some(p) => p,
        None => {
            let _ = env.throw_new("java/lang/RuntimeException", "Device not open");
            return 0;
        }
    };

    let new_hz = frequency_hz as u32;
    let offset = frequency_hz - p.manager.center_hz() as i64;

    if offset.abs() <= 1_000_000 {
        p.manager.set_ddc_offset(offset as f32);
        return 1;
    }

    match p.device.set_frequency(new_hz) {
        Ok(_) => {
            p.stream.mark_retuned();
            p.manager.note_hardware_retune(new_hz);
            2
        }
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", format!("setFrequency: {e}"));
            0
        }
    }
}

// tenths_db: gain in tenths of dB (e.g. 280 = 28.0 dB). Pass 0 for auto-gain.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_setGain(
    mut env: JNIEnv,
    _class: JClass,
    tenths_db: jint,
) -> jboolean {
    let mut lock = PIPELINE.lock();
    match lock.as_mut() {
        Some(p) => match p.device.set_gain(tenths_db, tenths_db == 0) {
            Ok(_) => 1,
            Err(e) => {
                let _ = env.throw_new("java/lang/RuntimeException", format!("setGain: {e}"));
                0
            }
        },
        None => {
            let _ = env.throw_new("java/lang/RuntimeException", "Device not open");
            0
        }
    }
}

// Returns a Java int[] of gain values in tenths of dB.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_getTunerGains(
    env: JNIEnv,
    _class: JClass,
) -> jni::sys::jintArray {
    let empty = env.new_int_array(0).unwrap().into_raw();
    let lock = PIPELINE.lock();
    let p = match lock.as_ref() {
        Some(p) => p,
        None => return empty,
    };
    let gains = p.device.available_gains();
    if gains.is_empty() {
        return empty;
    }
    match env.new_int_array(gains.len() as i32) {
        Ok(arr) => {
            let _ = env.set_int_array_region(&arr, 0, &gains);
            arr.into_raw()
        }
        Err(_) => empty,
    }
}

// Called by Kotlin audio thread on a tight loop.
// Returns interleaved stereo float PCM [L0, R0, L1, R1, …].
//
// Transition behaviour is handled entirely by PipelineManager.process_iq:
//   - RampingDown: IQ faded at input so IIR filters drain, PCM fades out
//   - Draining:    empty returned, stale IQ discarded here
//   - RampingUp:   IQ faded at input, PCM fades in
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_getAudioBuffer(
    env: JNIEnv,
    _class: JClass,
) -> jfloatArray {
    let empty = env.new_float_array(0).unwrap().into_raw();
    let mut lock = PIPELINE.lock();
    let p = match lock.as_mut() {
        Some(p) => p,
        None => return empty,
    };

    if p.stream.fill().is_err() {
        return empty;
    }

    let available = p.stream.available();
    if available == 0 {
        return empty;
    }

    let iq = p.stream.drain(available);

    p.waveform.update_iq(&iq);
    p.waveform.update_rms(&iq);

    let pcm = p.manager.process_iq(iq);

    if !pcm.is_empty() {
        p.waveform.update_audio(&pcm);
    }

    match env.new_float_array(pcm.len() as i32) {
        Ok(arr) => {
            let _ = env.set_float_array_region(&arr, 0, &pcm);
            arr.into_raw()
        }
        Err(_) => empty,
    }
}

// Returns 512 floats representing the IQ signal envelope (pre-demod).
// Returns empty if no new snapshot is available.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_getIqWaveform(
    env: JNIEnv,
    _class: JClass,
) -> jfloatArray {
    let empty = env.new_float_array(0).unwrap().into_raw();
    let mut lock = PIPELINE.lock();
    let p = match lock.as_mut() {
        Some(p) => p,
        None => return empty,
    };

    match p.waveform.take_iq_waveform() {
        Some(w) => match env.new_float_array(512) {
            Ok(arr) => {
                let _ = env.set_float_array_region(&arr, 0, &w);
                arr.into_raw()
            }
            Err(_) => empty,
        },
        None => empty,
    }
}

// Returns 512 floats representing the post-demod audio waveform.
// Returns empty if no new snapshot is available.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_getAudioWaveform(
    env: JNIEnv,
    _class: JClass,
) -> jfloatArray {
    let empty = env.new_float_array(0).unwrap().into_raw();
    let mut lock = PIPELINE.lock();
    let p = match lock.as_mut() {
        Some(p) => p,
        None => return empty,
    };

    match p.waveform.take_audio_waveform() {
        Some(w) => match env.new_float_array(512) {
            Ok(arr) => {
                let _ = env.set_float_array_region(&arr, 0, &w);
                arr.into_raw()
            }
            Err(_) => empty,
        },
        None => empty,
    }
}

// Used by AnalyzerScreen. Returns float[] of dBFS values.
// Reads raw IQ directly — always available regardless of demod mode or transition state.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_getSpectrum(
    env: JNIEnv,
    _class: JClass,
) -> jfloatArray {
    let empty = env.new_float_array(0).unwrap().into_raw();
    let mut lock = PIPELINE.lock();
    let p = match lock.as_mut() {
        Some(p) => p,
        None => return empty,
    };

    let needed = p.fft.size;
    if p.stream.available() < needed {
        return empty;
    }

    let iq = p.stream.drain(needed);
    let bins = p.fft.magnitude_spectrum(&iq);

    match env.new_float_array(bins.len() as i32) {
        Ok(arr) => {
            let _ = env.set_float_array_region(&arr, 0, &bins);
            arr.into_raw()
        }
        Err(_) => empty,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_setMode(
    mut env: JNIEnv,
    _class: JClass,
    mode: jint,
) -> jboolean {
    let mode = match mode {
        0 => PipelineMode::Wfm,
        1 => PipelineMode::Nfm,
        2 => PipelineMode::AmDsb,
        3 => PipelineMode::AmUsb,
        4 => PipelineMode::AmLsb,
        _ => {
            let _ = env.throw_new(
                "java/lang/RuntimeException",
                format!("Unknown mode: {mode}"),
            );
            return 0;
        }
    };
    let mut lock = PIPELINE.lock();
    match lock.as_mut() {
        Some(p) => {
            p.manager.switch_mode(mode);
            1
        }
        None => {
            let _ = env.throw_new("java/lang/RuntimeException", "Device not open");
            0
        }
    }
}

/// Set the AM audio IF filter bandwidth in Hz. No-op when not in an AM mode.
/// Use this to dial in SSB passband width (e.g. 2800.0 for voice SSB).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_setAmBandwidth(
    mut env: JNIEnv,
    _class: JClass,
    bandwidth_hz: jfloat,
) -> jboolean {
    let mut lock = PIPELINE.lock();
    match lock.as_mut() {
        Some(p) => {
            p.manager.set_am_bandwidth_hz(bandwidth_hz);
            1
        }
        None => {
            let _ = env.throw_new("java/lang/RuntimeException", "Device not open");
            0
        }
    }
}

/// Returns RMS signal strength in [0.0, 1.0] from the most recent IQ block.
/// Updated every getAudioBuffer() call. Returns 0.0 when no device is open.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_getSignalStrength(
    _env: JNIEnv,
    _class: JClass,
) -> jfloat {
    PIPELINE
        .lock()
        .as_ref()
        .map(|p| p.waveform.signal_strength())
        .unwrap_or(0.0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_isStereoDetected(
    _env: JNIEnv,
    _class: JClass,
) -> jboolean {
    PIPELINE
        .lock()
        .as_ref()
        .map(|p| p.manager.is_stereo_detected() as jboolean)
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_closeDevice(_env: JNIEnv, _class: JClass) {
    *PIPELINE.lock() = None;
}

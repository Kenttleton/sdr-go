#![allow(dead_code)]
#![allow(unused_imports)]

mod usb;
mod dsp;

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::sync::Arc;

use jni::JNIEnv;
use jni::objects::{JClass, JFloatArray, JObject};
use jni::sys::{jboolean, jfloat, jint, jlong, jstring, jfloatArray};

use usb::{SdrDevice, DeviceConfig, IqStream};
use dsp::{FmDemodulator, FmMode, FmAudioFrame};

// Global pipeline state — lives for the lifetime of the loaded library
struct Pipeline {
    device: SdrDevice,
    stream: IqStream,
    demodulator: FmDemodulator,
}

// SAFETY: Pipeline is guarded by Mutex — access is always single-threaded.
// The Send bound is required for Lazy<Mutex<Option<Pipeline>>> to be Sync.
unsafe impl Send for Pipeline {}

static PIPELINE: Lazy<Mutex<Option<Pipeline>>> =
    Lazy::new(|| Mutex::new(None));

// ─── Core version ────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_coreVersion(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let version = format!(
        "sdr_core v{} - Rust 1.92.0 - pipeline ready",
        env!("CARGO_PKG_VERSION"),
    );
    env.new_string(version)
        .expect("Failed to create string")
        .into_raw()
}

// ─── Open device ─────────────────────────────────────────────────────────────

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

    let fm_mode = if stereo != 0 {
        FmMode::StereoFallbackMono
    } else {
        FmMode::Mono
    };

    match SdrDevice::open_from_fd(fd, config.clone()) {
        Ok(device) => {
            let inner = device.inner();
            let stream = IqStream::new(inner);
            let demodulator = FmDemodulator::new(
                config.sample_rate,
                config.audio_sample_rate,
                fm_mode,
            );

            let mut pipeline = PIPELINE.lock();
            *pipeline = Some(Pipeline {
                device,
                stream,
                demodulator,
            });

            1 // true
        }
        Err(e) => {
            let _ = env.throw_new(
                "java/lang/RuntimeException",
                format!("Failed to open SDR device: {}", e),
            );
            0 // false
        }
    }
}

// ─── Set frequency ────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_setFrequency(
    mut env: JNIEnv,
    _class: JClass,
    frequency_hz: jlong,
) -> jboolean {
    let mut pipeline = PIPELINE.lock();
    match pipeline.as_mut() {
        Some(p) => {
            match p.device.set_frequency(frequency_hz as u32) {
                Ok(_) => 1,
                Err(e) => {
                    let _ = env.throw_new(
                        "java/lang/RuntimeException",
                        format!("Failed to set frequency: {}", e),
                    );
                    0
                }
            }
        }
        None => {
            let _ = env.throw_new(
                "java/lang/RuntimeException",
                "Device not open",
            );
            0
        }
    }
}

// ─── Get audio buffer ─────────────────────────────────────────────────────────
// Called by Kotlin audio thread on a tight loop
// Returns interleaved stereo float PCM [L0, R0, L1, R1, ...]
// or mono [S0, S1, S2, ...] depending on detected signal

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_getAudioBuffer(
    env: JNIEnv,
    _class: JClass,
) -> jfloatArray {
    let mut pipeline = PIPELINE.lock();

    let empty = env.new_float_array(0)
        .expect("Failed to create empty array")
        .into_raw();

    let p = match pipeline.as_mut() {
        Some(p) => p,
        None => return empty,
    };

    // Fill ring buffer from USB
    if p.stream.fill().is_err() {
        return empty;
    }

    // Drain available samples
    let available = p.stream.available();
    if available == 0 {
        return empty;
    }

    let iq = p.stream.drain(available);

    // Demodulate
    let frame = p.demodulator.process(&iq);

    // Serialize to interleaved float array for Kotlin
    let pcm: Vec<f32> = match frame {
        FmAudioFrame::Mono(samples) => {
            // Duplicate mono to both channels for AudioTrack stereo output
            samples.iter().flat_map(|&s| [s, s]).collect()
        }
        FmAudioFrame::Stereo(left, right) => {
            // Interleave L/R
            left.iter().zip(right.iter())
                .flat_map(|(&l, &r)| [l, r])
                .collect()
        }
    };

    // Create Java float array and copy PCM into it
    match env.new_float_array(pcm.len() as i32) {
        Ok(arr) => {
            let _ = env.set_float_array_region(&arr, 0, &pcm);
            arr.into_raw()
        }
        Err(_) => empty,
    }
}

// ─── Is stereo detected ───────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_isStereoDetected(
    _env: JNIEnv,
    _class: JClass,
) -> jboolean {
    let pipeline = PIPELINE.lock();
    match pipeline.as_ref() {
        Some(p) => p.demodulator.is_stereo_detected() as jboolean,
        None => 0,
    }
}

// ─── Close device ─────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_closeDevice(
    _env: JNIEnv,
    _class: JClass,
) {
    let mut pipeline = PIPELINE.lock();
    *pipeline = None; // Drop closes the device
}
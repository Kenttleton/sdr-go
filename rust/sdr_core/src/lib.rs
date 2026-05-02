#![allow(dead_code)]
#![allow(unused_imports)]

mod dsp;
mod usb;

use once_cell::sync::Lazy;
use parking_lot::Mutex;

use jni::objects::{JClass, JFloatArray, JIntArray, JObject, JString};
use jni::sys::{jboolean, jfloat, jfloatArray, jint, jintArray, jlong, jstring};
use jni::JNIEnv;

use dsp::{
    design_low_pass, normalize_freq, FMAudioFrame, FMDemodulationMode, FMDemodulator, Window,
};
use usb::{DeviceConfig, IqStream, SdrDevice};

// ── Global pipeline ────────────────────────────────────────────────────────────

struct Pipeline {
    device: SdrDevice,
    stream: IqStream,
    demodulator: FMDemodulator,
    gain_tenths: i32,
    auto_gain: bool,
}

// SAFETY: Pipeline is always accessed through the Mutex — never aliased.
unsafe impl Send for Pipeline {}

static PIPELINE: Lazy<Mutex<Option<Pipeline>>> = Lazy::new(|| Mutex::new(None));

// ── Helpers ────────────────────────────────────────────────────────────────────

fn escape_json_str(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

const cutoff: f32 = normalize_freq(15_000.0, 240_000.0); // 15kHz audio LPF at 240k IQ
const lpf: FIRFilter = FIRFilter::new(design_low_pass(cutoff, 128, Window::Hamming));

// ── Core version ───────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_coreVersion(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let version = format!(
        "sdr_core v{} — Rust/JNI — RDS+EQ+Stereo",
        env!("CARGO_PKG_VERSION"),
    );
    env.new_string(version).expect("string").into_raw()
}

// ── Open device ────────────────────────────────────────────────────────────────

fn init_logger() {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("sdr_core"),
    );
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_openDevice(
    mut env: JNIEnv,
    _class: JClass,
    fd: jint,
    frequency_hz: jlong,
    stereo: jboolean,
    stations_mode: jboolean,
) -> jboolean {
    init_logger();

    // Stations mode: 1.2 MSPS hardware rate (stable RTL-SDR range: 900k–3.2M).
    // 240 kHz was in the unstable 225k–300k range causing USB packet corruption.
    // A 64-tap channel-selection LPF (cutoff 0.1 normalized = 120 kHz) attenuates
    // adjacent channels before 5× IQ decimation to 240 kHz intermediate rate.
    // 240 kHz intermediate → 48 kHz audio (5× more); RDS at 57 kHz fits within
    // the 120 kHz Nyquist. Wide mode keeps 2.048 MSPS for max audio quality.
    let (sample_rate, audio_rate) = if stations_mode != 0 {
        (1_200_000u32, 48_000u32)
    } else {
        (2_048_000u32, 96_000u32)
    };

    let config = DeviceConfig {
        frequency_hz: frequency_hz as u32,
        sample_rate,
        audio_sample_rate: audio_rate,
        gain_tenths: None, // device default (auto gain); user can override via setGain()
        ..Default::default()
    };

    let fm_mode = if stereo != 0 {
        FMDemodulationMode::Mono
    };

    match SdrDevice::open(fd, config.clone()) {
        Ok(device) => {
            let inner = device.inner();
            let stream = IqStream::new(inner, device.bulk_transfer_samples(), None);
            let demodulator = if stations_mode != 0 {
                FMDemodulator::new_stations(sample_rate, audio_rate, fm_mode)
            } else {
                FMDemodulator::new(sample_rate, audio_rate, fm_mode)
            };

            let mut pipeline = PIPELINE.lock();
            *pipeline = Some(Pipeline {
                device,
                stream,
                demodulator,
                gain_tenths: 0,
                auto_gain: true,
            });
            1
        }
        Err(e) => {
            let _ = env.throw_new(
                "java/lang/RuntimeException",
                format!("Failed to open SDR device: {}", e),
            );
            0
        }
    }
}

// ── Tune frequency ─────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_setFrequency(
    mut env: JNIEnv,
    _class: JClass,
    frequency_hz: jlong,
) -> jboolean {
    let mut pipeline = PIPELINE.lock();
    match pipeline.as_mut() {
        Some(p) => match p.device.set_frequency(frequency_hz as u32) {
            Ok(_) => 1,
            Err(e) => {
                let _ = env.throw_new("java/lang/RuntimeException", format!("setFrequency: {}", e));
                0
            }
        },
        None => {
            let _ = env.throw_new("java/lang/RuntimeException", "Device not open");
            0
        }
    }
}

// ── Audio buffer ───────────────────────────────────────────────────────────────
// Returns interleaved stereo float PCM: [L0, R0, L1, R1, ...]

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_getAudioBuffer(
    env: JNIEnv,
    _class: JClass,
) -> jfloatArray {
    let empty = env.new_float_array(0).expect("array").into_raw();

    let mut pipeline = PIPELINE.lock();
    let p = match pipeline.as_mut() {
        Some(p) => p,
        None => return empty,
    };

    match p.stream.fill() {
        Ok(n) => log::debug!("getAudioBuffer: fill ok, {} IQ samples", n),
        Err(e) => {
            log::warn!("getAudioBuffer: fill error — {}", e);
            return empty;
        }
    }

    let available = p.stream.available();
    if available == 0 {
        return empty;
    }

    let iq = p.stream.drain(available);
    log::debug!(
        "getAudioBuffer: draining {} IQ samples, signal_power={:.4}",
        iq.len(),
        p.demodulator.signal_power
    );
    let pcm: Vec<f32> = match p.demodulator.process(&iq) {
        FMAudioFrame::Mono(s) => s.iter().flat_map(|&x| [x, x]).collect(),
        FMAudioFrame::Stereo(l, r) => l
            .iter()
            .zip(r.iter())
            .flat_map(|(&lv, &rv)| [lv, rv])
            .collect(),
    };

    match env.new_float_array(pcm.len() as i32) {
        Ok(arr) => {
            let _ = env.set_float_array_region(&arr, 0, &pcm);
            arr.into_raw()
        }
        Err(_) => empty,
    }
}

// ── Stereo detection ───────────────────────────────────────────────────────────

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

// ── Signal strength ────────────────────────────────────────────────────────────
// Returns IQ power estimate in [0, 1]. Useful for squelch and scan threshold.

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_nativeGetSignalStrength(
    _env: JNIEnv,
    _class: JClass,
) -> jfloat {
    let pipeline = PIPELINE.lock();
    match pipeline.as_ref() {
        Some(p) => (p.demodulator.signal_power * 5.0).min(1.0) as jfloat,
        None => 0.0,
    }
}

// ── RDS info ───────────────────────────────────────────────────────────────────
// Returns a JSON string or empty string when not in stations mode / no data yet.

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_nativeGetRdsInfo(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let pipeline = PIPELINE.lock();
    let json = match pipeline.as_ref().and_then(|p| p.demodulator.rds()) {
        Some(rds) => {
            format!(
                r#"{{"pi":{},"ps":"{}","rt":"{}","pty":{},"tp":{},"ta":{},"ms":{},"psReady":{},"rtReady":{}}}"#,
                rds.pi,
                escape_json_str(&rds.ps_string()),
                escape_json_str(&rds.rt_string()),
                rds.pty,
                rds.tp,
                rds.ta,
                rds.ms,
                rds.ps_ready,
                rds.rt_ready,
            )
        }
        None => String::from("{}"),
    };

    env.new_string(&json).expect("string").into_raw()
}

// ── Stream flush ───────────────────────────────────────────────────────────────
// Discards buffered IQ samples. Intended for disconnect cleanup before the
// hardware handle is released.

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_nativeFlushStream(_env: JNIEnv, _class: JClass) {
    let mut pipeline = PIPELINE.lock();
    if let Some(p) = pipeline.as_mut() {
        p.stream.flush();
    }
}

// ── Hardware gain control ──────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_nativeSetGain(
    mut env: JNIEnv,
    _class: JClass,
    tenths_db: jint,
    auto_gain: jboolean,
) -> jboolean {
    let mut pipeline = PIPELINE.lock();
    match pipeline.as_mut() {
        Some(p) => match p.device.set_gain(tenths_db, auto_gain != 0) {
            Ok(_) => {
                p.gain_tenths = tenths_db;
                p.auto_gain = auto_gain != 0;
                1
            }
            Err(e) => {
                let _ = env.throw_new("java/lang/RuntimeException", format!("setGain: {}", e));
                0
            }
        },
        None => {
            let _ = env.throw_new("java/lang/RuntimeException", "Device not open");
            0
        }
    }
}

/// Returns available gain values in tenths of dB as a Java int array.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_nativeGetAvailableGains(
    env: JNIEnv,
    _class: JClass,
) -> jintArray {
    let empty = env.new_int_array(0).expect("array").into_raw();
    let pipeline = PIPELINE.lock();
    match pipeline.as_ref() {
        Some(p) => {
            let gains = p.device.available_gains();
            match env.new_int_array(gains.len() as i32) {
                Ok(arr) => {
                    let _ = env.set_int_array_region(&arr, 0, &gains);
                    arr.into_raw()
                }
                Err(_) => empty,
            }
        }
        None => empty,
    }
}

// ── EQ ─────────────────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_nativeSetEq(
    env: JNIEnv,
    _class: JClass,
    bands: JFloatArray,
) -> jboolean {
    let mut buf = [0.0f32; 7];
    match env.get_float_array_region(&bands, 0, &mut buf) {
        Ok(_) => {
            let mut pipeline = PIPELINE.lock();
            if let Some(p) = pipeline.as_mut() {
                p.demodulator.set_eq(&buf);
            }
            1
        }
        Err(_) => 0,
    }
}

// ── Mono mode ──────────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_nativeSetMonoMode(
    _env: JNIEnv,
    _class: JClass,
    mono: jboolean,
) -> jboolean {
    let mut pipeline = PIPELINE.lock();
    if let Some(p) = pipeline.as_mut() {
        let mode = if mono != 0 {
            FMDemodulationMode::Mono
        };
        p.demodulator.set_mode(mode);
        1
    } else {
        0
    }
}

// ── Close device ───────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_closeDevice(_env: JNIEnv, _class: JClass) {
    let mut pipeline = PIPELINE.lock();
    if let Some(p) = pipeline.as_mut() {
        p.stream.flush(); // discard buffered IQ samples
        p.device.close(); // release USB hardware handle through shared Arc
    }
    *pipeline = None;
}

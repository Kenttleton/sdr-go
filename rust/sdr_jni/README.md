# sdr_jni

Thin JNI wrapper that exposes `sdr_core` to Android. Compiles to a `cdylib` named `libsdr_core.so` and is loaded by the React Native `SdrModule` on Android.

## Overview

- **Java class:** `com.sdrgo.SdrModule`
- **Output artifact:** `libsdr_core.so` (cdylib)
- **Dependencies:** `sdr_core`, `jni 0.21`, `parking_lot`, `once_cell`
- **State model:** single global `Pipeline` instance guarded by a `Mutex`. All methods are safe to call from any thread; the JNI layer serializes access.

## JNI Methods

### Device lifecycle

| Java method | Signature | Returns | Notes |
|---|---|---|---|
| `coreVersion` | `()` | `String` | `"sdr_core vX.Y.Z · futuredsp · rustfft"` |
| `openDevice` | `(fd: int, frequency_hz: long, audio_sample_rate: int, stereo: boolean)` | `boolean` | Opens device via Android USB fd; initializes the full pipeline. Throws `RuntimeException` on failure. |
| `closeDevice` | `()` | `void` | Drops the pipeline and releases the device. |

### Tuning & gain

| Java method | Signature | Returns | Notes |
|---|---|---|---|
| `setFrequency` | `(frequency_hz: long)` | `int` | `0` = error, `1` = DDC software tune (offset ≤ 1 MHz), `2` = hardware retune (settling started). |
| `setGain` | `(tenths_db: int)` | `boolean` | Gain in tenths of dB (e.g. `280` = 28.0 dB). Pass `0` for auto-gain. |
| `getTunerGains` | `()` | `int[]` | Available gain steps in tenths of dB. Returns empty array if device is not open. |

### Demodulation mode

| Java method | Signature | Returns | Notes |
|---|---|---|---|
| `setMode` | `(mode: int)` | `boolean` | `0`=WFM, `1`=NFM, `2`=AM-DSB, `3`=AM-USB, `4`=AM-LSB. Throws on unknown mode. |
| `setAmBandwidth` | `(bandwidth_hz: float)` | `boolean` | Adjusts AM IF filter width in Hz. |

### Audio & signal data

| Java method | Signature | Returns | Notes |
|---|---|---|---|
| `getAudioBuffer` | `()` | `float[]` | Pulls the next IQ batch through the full demodulation pipeline; returns PCM samples. Call on each audio render tick. |
| `getIqWaveform` | `()` | `float[]` | 512 interleaved I/Q samples for oscilloscope display. |
| `getAudioWaveform` | `()` | `float[]` | 512 audio samples for waveform display. |
| `getSpectrum` | `()` | `float[]` | 2048-point FFT magnitude spectrum in dBFS. |
| `getSignalStrength` | `()` | `float` | Normalized signal strength `[0.0, 1.0]`. |
| `getRssi` | `()` | `float` | RSSI in dBFS. Returns `-100.0` when no device is open. |

### Squelch & stereo

| Java method | Signature | Returns | Notes |
|---|---|---|---|
| `setSquelch` | `(threshold_db: float, hang_ms: float)` | `void` | Opens audio when RSSI > `threshold_db`; holds open for `hang_ms` after signal drops. |
| `isStereoDetected` | `()` | `boolean` | `true` when the WFM stereo pilot tone is locked. |

## Building for Android

Build from the workspace root via the existing `build-rust.sh` script, which sets the correct NDK toolchain and copies the `.so` into the Android project's jniLibs directory.

```sh
./scripts/build-rust.sh
```

Targets: `aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android`.

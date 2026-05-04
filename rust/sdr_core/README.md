# sdr_core

Rust signal processing library for RTL-SDR receivers on Android. Handles USB device communication, IQ streaming, and real-time demodulation through a JNI boundary for Kotlin native modules.

Part of the [SDRGo](https://github.com/kenttleton/sdr-core) project.

---

## Status

| Feature | Status |
|---|---|
| RTL-SDR Blog V3 USB | ✅ Working |
| FM Wide (WFM) — stereo detect | ✅ Working |
| FM Narrow (NFM) | 🔜 Planned |
| AM — envelope (DSB) | ✅ Working |
| AM — SAM / SSB (USB, LSB) | 🚧 Stubbed |
| Spectrum analyzer (FFT) | ✅ Working |
| IQ / audio waveform display | ✅ Working |
| Smooth frequency transitions (DDC) | ✅ Working |
| Smooth mode transitions (crossfade) | ✅ Working |
| Parametric EQ | 🔜 Planned |
| RDS decoder (WFM) | 🔜 Planned |
| Signal strength (RMS) | 🔜 Planned |

---

## Architecture

```
React Native / Expo          UI layer
        ↕ TypeScript
Kotlin SdrModule             AudioTrack, USB permissions, scan engine
        ↕ JNI
sdr_core (this library)      IQ streaming, demodulation, DSP, spectrum
        ↕ libusb
RTL-SDR Hardware             Raw IQ samples over USB OTG
```

### Module structure

```
src/
  lib.rs          JNI entry point and pipeline orchestration
  usb/
    device.rs     SdrDevice — open, tune, gain, bulk transfer
    stream.rs     IqStream — ring buffer, settling discard after retune
    hardware.rs   RTL-SDR register configuration (R820T2 tuner, RTL2832U demod)
  pipeline/
    manager.rs    PipelineManager — state machine, DDC, mode switching, crossfade
    fm.rs         FmPipeline — polar discriminator, pre-filter, stereo pilot, AGC
    am.rs         AmPipeline — envelope detect, IF filter, bandwidth control, AGC
    ddc.rs        Ddc — digital down-converter for glitch-free sub-MHz retuning
    filters.rs    FIR filter, decimating FIR, Kaiser window, firdes helpers
    spectrum.rs   FftStage (magnitude spectrum) + WaveformStage (display snapshots)
    mod.rs        DemodPipeline enum — dispatches to FM / AM
```

---

## Build

### Prerequisites

```bash
rustup target add aarch64-linux-android x86_64-linux-android
cargo install cargo-ndk
```

NDK **27.1.12297006** required. Set `ANDROID_NDK_HOME` to the NDK path before building.

### Build for Android

```bash
cd rust/sdr_core
cargo ndk -t aarch64-linux-android -t x86_64-linux-android build --release
```

Or use the project build script from the repo root:

```bash
./scripts/build-rust.sh
```

---

## JNI API

All functions follow the Android JNI naming convention:

```
Java_com_sdrgo_SdrModule_<methodName>
```

### Device lifecycle

| Kotlin method | Returns | Description |
|---|---|---|
| `coreVersion()` | `String` | Library version string — use to verify the native library loaded correctly |
| `openDevice(fd, frequencyHz, audioSampleRate, stereo)` | `Boolean` | Open USB file descriptor and initialize the pipeline. `audioSampleRate` sets the PCM output rate (48 000 or 96 000 Hz). `stereo` enables stereo decode for WFM |
| `closeDevice()` | `void` | Tear down the pipeline and release the USB device |

### Tuning

| Kotlin method | Returns | Description |
|---|---|---|
| `setFrequency(frequencyHz)` | `Int` | 0 = error, 1 = DDC software tune (glitch-free, ≤ ±1 MHz offset), 2 = hardware retune (settling started) |

### Gain

| Kotlin method | Returns | Description |
|---|---|---|
| `setGain(tenthsDb)` | `Boolean` | Hardware gain in tenths of dB (e.g. 280 = 28.0 dB). Pass 0 for auto-gain (AGC) |
| `getTunerGains()` | `IntArray` | Available hardware gain steps in tenths of dB |

### Mode

| Kotlin method | Returns | Description |
|---|---|---|
| `setMode(mode)` | `Boolean` | 0 = WFM, 1 = NFM, 2 = AM-DSB, 3 = AM-USB, 4 = AM-LSB. Transitions are crossfaded |
| `isStereoDetected()` | `Boolean` | True when a 19 kHz pilot tone is present in the signal (WFM only) |

### Audio

| Kotlin method | Returns | Description |
|---|---|---|
| `getAudioBuffer()` | `FloatArray` | Interleaved stereo float PCM `[L0, R0, L1, R1, …]`. Call from a tight loop; drives AudioTrack |

### Display outputs

| Kotlin method | Returns | Description |
|---|---|---|
| `getIqWaveform()` | `FloatArray` | 512-sample IQ envelope snapshot, captured pre-demod. Empty array if no new data since last call |
| `getAudioWaveform()` | `FloatArray` | 512-sample audio PCM snapshot, captured post-demod. Empty array if no new data since last call |
| `getSpectrum()` | `FloatArray` | Hann-windowed magnitude spectrum in dBFS, 1024 bins. Empty array if insufficient IQ buffered |

---

## Supported Hardware

| Device | Status |
|---|---|
| RTL-SDR Blog V3 | ✅ Working |
| RTL-SDR Blog V4 | 🔜 Planned |
| NooElec NESDR SMArt | 🔜 Untested |
| HackRF One | 🔜 Future |
| Airspy | 🔜 Future |

---

## Roadmap

- **NFM** — narrow-band FM for voice and utility bands; no stereo, no RDS
- **RDS decoder (WFM)** — 57 kHz subcarrier extraction, BPSK demod, clock recovery, CRC-10 group decode → PS / RT / PTY / PI
- **Parametric EQ** — up to 7 bands (bell peak + low/high shelf filters) applied at audio rate inside the Rust pipeline
- **Signal strength (RMS)** — IQ power measurement captured as a parallel output alongside the waveform snapshots
- **AM SAM / SSB** — synchronous AM and single-sideband modes (infrastructure in place, demod logic stubbed)
- **AdaptiveFirFilter** — crossfade between filter coefficients during bandwidth changes to eliminate click artifacts

---

## License

Licensed under either of:

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option. Both licenses are permissive and impose no restrictions on commercial or closed-source use.

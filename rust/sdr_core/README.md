# sdr_core

Pure Rust signal processing library for RTL-SDR receivers. Handles USB device
communication, IQ streaming, and real-time demodulation. Platform-agnostic core
used by two entry points:

- **`sdr_jni`** — thin `cdylib` wrapper exposing the library to Android via JNI
- **`sdr_srv`** — network binary serving the pipeline over HTTP/WebSocket

Part of the [SDRGo](https://github.com/kenttleton/sdr-core) project.

---

## Status

| Feature | Status |
|---|---|
| RTL-SDR Blog V3 USB | ✅ Working |
| FM Wide (WFM) — stereo detect | ✅ Working |
| FM Narrow (NFM) | ✅ Working |
| AM — envelope (DSB) | ✅ Working |
| AM — SAM / SSB (USB, LSB) | 🚧 Stubbed |
| Spectrum analyzer (FFT) | ✅ Working |
| IQ / audio waveform display | ✅ Working |
| Smooth frequency transitions (DDC) | ✅ Working |
| Smooth mode transitions (crossfade) | ✅ Working |
| Signal strength (RMS of raw IQ) | ✅ Working |
| Parametric EQ | 🔜 Planned |
| RDS decoder (WFM) | 🔜 Planned |

---

## Workspace layout

```
rust/
  sdr_core/      this crate — pure rlib (pipeline, service, usb)
  sdr_jni/       cdylib wrapper → Android .so via cargo-ndk
  sdr_srv/       binary → HTTP/WebSocket server (Linux/macOS)
```

## Architecture

```
Android app                  sdr_srv (Linux/macOS)
     ↕ JNI                        ↕ HTTP + WebSocket
  sdr_jni                      sdr_srv
     ↕                             ↕
           sdr_core  ←  you are here
     ↕ usb::DeviceSource
RTL-SDR hardware          RTL-SDR hardware
```

### Module structure

```
src/
  lib.rs          Re-exports pipeline, service, usb
  service.rs      RadioService + RadioServiceHandle — lock-free DSP ↔ control bridge
  usb/
    device.rs     SdrDevice, DeviceSource, DeviceConfig, DeviceInfo
    stream.rs     IqStream — ring buffer, settling discard after retune
    hardware.rs   RtlSdrHardware — R820T2 tuner, RTL2832U demod
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

## Opening a device

`DeviceSource` covers all libusb-capable open paths:

| Variant | Platforms | Notes |
|---|---|---|
| `FileDescriptor { fd, no_discovery: true }` / `android_fd(fd)` | Android | FD from Java USB host API; suppresses libusb bus scan |
| `FileDescriptor { fd, no_discovery: false }` / `posix_fd(fd)` | Linux / embedded | FD from a `/dev/bus/usb/…` device node |
| `FirstAvailable` | Linux / macOS | Requires `enumerate` feature |
| `Index(usize)` | Linux / macOS | Requires `enumerate` feature |
| `Serial(String)` | Linux / macOS | Requires `enumerate` feature |
| `VidPid { vid, pid }` | Linux / macOS | Requires `enumerate` feature |

`FileDescriptor` is gated to `#[cfg(unix)]` — Windows is not currently supported
(see Roadmap).

---

## Build

### Android

```bash
rustup target add aarch64-linux-android x86_64-linux-android
cargo install cargo-ndk
```

NDK **27.1.12297006** required. Set `ANDROID_NDK_HOME` before building.

```bash
# From workspace root:
cargo ndk -t aarch64-linux-android -t x86_64-linux-android build --release -p sdr_jni

# Or via the project build script:
./scripts/build-rust.sh
```

### Linux / macOS (`sdr_srv`)

```bash
# From workspace root:
cargo build --release -p sdr_srv
```

The `enumerate` feature is enabled automatically for `sdr_srv`. libusb must be
available (`brew install libusb` / `apt install libusb-1.0-0-dev`).

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

- **AM SAM / SSB** — synchronous AM and single-sideband modes (infrastructure in place, demod logic stubbed)
- **RDS decoder (WFM)** — 57 kHz subcarrier extraction, BPSK demod, clock recovery, CRC-10 group decode → PS / RT / PTY / PI
- **Parametric EQ** — up to 7 bands (bell peak + low/high shelf) applied at audio rate inside the Rust pipeline
- **AdaptiveFirFilter** — crossfade between filter coefficients during bandwidth changes to eliminate click artifacts
- **RTL-SDR Blog V4** — bias-tee, HF direct sampling, updated register map
- **Windows support** — blocked on the libusb dependency. Path forward: replace the `rtl-sdr-rs` transport layer with [`nusb`](https://crates.io/crates/nusb) (pure Rust, no libusb, WinUSB built into Windows 10+). Requires implementing the RTL2832U register protocol directly on `nusb`. When complete, Windows users would need only a one-time Zadig driver switch, not a system libusb install.

---

## License

Licensed under either of:

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.

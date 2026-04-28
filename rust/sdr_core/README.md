# sdr_core

A modular, high-performance SDR (Software Defined Radio) signal processing library for Android, written in Rust. Built as the foundation of [SDRGo](https://github.com/utterback/sdrgo) and designed to be reusable by any Android SDR application.

Solves the hard problem of RTL-SDR hardware communication and real-time signal processing on Android in a memory-safe, well-structured library that other developers can build on.

---

## Status

> 🚧 Active development — API is not yet stable. Version 1.0.0 will mark API stability.

| Module | Status |
|---|---|
| USB / Hardware Abstraction | 🚧 In progress |
| IQ Stream Pipeline | 🚧 In progress |
| DSP Primitives | 🔜 Planned |
| FM Demodulator | 🔜 Planned |
| AM / NFM Demodulator | 🔜 Planned |
| NOAA Weather Audio | 🔜 Planned |
| NOAA APT Satellite Imagery | 🔜 Planned |
| ADS-B Decoder | 🔜 Planned |
| Scan Engine | 🔜 Planned |
| Audio Output | 🔜 Planned |
| RDS Decoder | 🔜 Planned |

---

## Architecture

`sdr_core` is the native signal processing layer in a three-tier Android SDR stack:

```
React Native / Expo          UI layer
        ↕
Kotlin Native Module         Android APIs, AudioTrack, USB permissions, JNI bridge
        ↕
sdr_core (this library)      Signal processing, decoding, pipeline orchestration
        ↕
C libraries via FFI          librtlsdr, liquid-dsp, dump1090
        ↕
RTL-SDR Hardware             Raw IQ samples over USB OTG
```

### Module Structure

```
sdr_core/
  src/
    usb/          Hardware abstraction — device enumeration, init, IQ streaming
    dsp/          DSP primitives — filters, FFT, decimation, AGC
    demod/        Demodulators — FM, AM, NFM, SSB (each pluggable)
    decode/       Decoders — ADS-B, NOAA APT, RDS, SAME/EAS
    pipeline/     Orchestration — threading model, ring buffer, scheduler
    audio/        Audio output — PCM buffer management, sample rate conversion
    jni/          JNI boundary — all extern "C" functions exposed to Kotlin
```

### Threading Model

```
USB read thread      Highest priority — feeds ring buffer from dongle
DSP thread           Consumes ring buffer — runs demodulation
Decode thread        Runs heavier decoders (ADS-B, APT)
Audio thread         Feeds Android AudioTrack
Callback thread      Pushes decoded data events to Kotlin via JNI
```

---

## Supported Hardware

| Hardware | Status |
|---|---|
| RTL-SDR Blog V3 | 🚧 In progress |
| RTL-SDR Blog V4 | 🔜 Planned |
| NooElec NESDR SMArt | 🔜 Planned |
| HackRF One | 🔜 Future |
| Airspy | 🔜 Future |

---

## Supported Bands and Modes

| Band | Frequency | Mode | Status |
|---|---|---|---|
| FM Broadcast | 87.5 – 108 MHz | WFM | 🔜 Planned |
| NOAA Weather | 162.400 – 162.550 MHz | NFM + SAME/EAS | 🔜 Planned |
| VHF Air Band | 118 – 137 MHz | AM + Squelch | 🔜 Planned |
| ADS-B | 1090 MHz | Mode S decode | 🔜 Planned |
| NOAA Satellites | 137.500 – 137.912 MHz | APT imagery | 🔜 Planned |
| AM Broadcast | 520 – 1710 kHz | AM | 🔜 Planned |
| Shortwave | 1.7 – 30 MHz | AM / SSB | 🔜 Planned |

---

## Using sdr_core in Your Project

### As a Cargo dependency (once published)

```toml
[dependencies]
sdr_core = "0.1.0"
```

### During development (path dependency)

```toml
[dependencies]
sdr_core = { path = "../sdr-core" }
```

### Android NDK integration

`sdr_core` compiles to a `.so` via `cargo-ndk` and is linked into your Android app via CMake. See the [SDRGo integration example](https://github.com/utterback/sdrgo) for a complete working setup including CMakeLists.txt and Kotlin JNI bridge.

### Prerequisites

```bash
# Install Android targets
rustup target add \
  aarch64-linux-android \
  armv7-linux-androideabi \
  x86_64-linux-android \
  i686-linux-android

# Install cargo-ndk
cargo install cargo-ndk

# Build for Android
cargo ndk \
  -t aarch64-linux-android \
  -t x86_64-linux-android \
  build --release
```

---

## JNI API Reference

All functions exposed to Kotlin are in `src/jni/`. They follow Android JNI naming convention:

```
Java_{package}_{ClassName}_{methodName}
```

### Core

| Kotlin Method | Description |
|---|---|
| `SdrModule.coreVersion()` | Returns library version string — use to verify pipeline is wired correctly |

*More methods will be documented here as modules are completed.*

---

## Expo / React Native module wrapper (planned)

When the driver API is stable, `sdr_core` will be wrapped as a standalone Expo module so it can be consumed by any Expo or bare React Native app via `npx expo install sdr-core`.

### Why a module wrapper

Currently the Rust library is integrated directly into the SDRGo app — the app owns the CMake build, the Kotlin bridge, and the TypeScript wrapper. That works for a single app but isn't distributable. An Expo module package moves all of that into a self-contained npm package that handles its own native build, so consumers get everything with one install command.

### Steps to create the module

1. **Scaffold the package**

   ```bash
   npx create-expo-module sdr-core --no-example
   ```

2. **Move the Rust source** into `sdr-core/rust/` alongside `Cargo.toml` and `src/lib.rs`

3. **Move the Kotlin bridge** — copy `SdrModule.kt` into the module's Android source tree; migrate from `ReactContextBaseJavaModule` to expo-modules-core's `Module` class so Expo handles registration automatically

4. **Move the CMake setup** — the module's `android/CMakeLists.txt` owns the Rust IMPORTED library and the `ReactNative-application.cmake` include; the app's `build.gradle` drops its `externalNativeBuild` block entirely

5. **Move the TypeScript wrapper** — `SdrModule.ts` (including the `driverError` availability check and all public methods) becomes the module's JS entry point

6. **Add a build hook** — write an Expo config plugin (`plugin/src/index.ts`) that runs `build-rust.sh` for the target ABI as part of `expo prebuild`, so the `.so` is always compiled before the Android build starts

7. **Publish to npm** — update the SDRGo app to `npx expo install sdr-core` and import from the package instead of the local `./src/modules/SdrModule`

### Pure native Android consumers

The Kotlin JNI bridge and compiled `.so` can be distributed as an Android AAR independently of the React Native layer. The JNI function signatures in `src/lib.rs` are stable C ABI and have no React Native dependency.

---

## Publishing to crates.io

When the API reaches stability (v1.0.0):

1. Ensure `Cargo.toml` metadata is complete — `description`, `repository`, `keywords`, `categories`, `license`
2. Verify `README.md` and `LICENSE-MIT` / `LICENSE-APACHE` are present
3. Do a dry run to catch any issues:
```bash
cargo publish --dry-run
```
4. Login to crates.io with your GitHub account:
```bash
cargo login
```
5. Publish:
```bash
cargo publish
```

Suggested crates.io keywords: `sdr`, `rtl-sdr`, `android`, `signal-processing`, `radio`

Suggested crates.io categories: `embedded`, `network-programming`, `science`

> Note: Once published, a version cannot be deleted — only yanked. Take the dry run seriously.

---

## Contributing

Contributions welcome once the core API stabilizes. Until v1.0.0 the architecture is still evolving and large PRs may conflict with in-progress work. Opening an issue to discuss first is recommended.

---

## License

Licensed under either of:

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.

This dual license is standard in the Rust ecosystem and imposes no restrictions on use in commercial or closed-source applications. If you modify and redistribute `sdr_core` itself, you may choose either license.

### Why dual MIT/Apache 2.0?

MIT is simple and maximally permissive. Apache 2.0 adds an explicit patent grant protecting users from patent claims related to the library. Offering both lets downstream users choose whichever is compatible with their project's license requirements.
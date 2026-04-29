# SDRGo

A suite of RTL-SDR Android apps built with React Native/Expo and a Rust signal processing core. One shared Android project, five app flavors, one Rust library.

| App | Flavor | Description |
|---|---|---|
| `apps/analyzer` | `analyzer` | Spectrum analyzer |
| `apps/fm` | `fm` | FM broadcast receiver |
| `apps/air` | `air` | VHF airband AM receiver |
| `apps/ham` | `ham` | Ham radio |
| `apps/noaa` | `noaa` | NOAA weather + satellite |

---

## Prerequisites

| Tool | Version | Install |
|---|---|---|
| Node.js | 20+ | [nodejs.org](https://nodejs.org) |
| npm | 11+ | bundled with Node |
| Java (JDK) | 17 | Android Studio or `brew install --cask temurin@17` |
| Android Studio | Latest | [developer.android.com/studio](https://developer.android.com/studio) |
| Android SDK | API 35 | via Android Studio SDK Manager |
| Android NDK | 27.1.12297006 | via Android Studio SDK Manager |
| Rust | stable | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh` |
| cargo-ndk | latest | `cargo install cargo-ndk` |

After installing Rust, add the Android targets:

```bash
rustup target add \
  aarch64-linux-android \
  x86_64-linux-android \
  armv7-linux-androideabi \
  i686-linux-android
```

---

## Setup

```bash
# 1. Install JS dependencies
npm install

# 2. Build the Rust native library (required before any Android build)
npm run build:rust
```

`build:rust` compiles `rust/sdr_core` for all Android ABIs via `cargo-ndk` and writes the `.so` files to `android/app/src/main/jniLibs/`. This only needs to re-run when Rust source changes.

---

## Development (Metro + hot reload)

Start the Metro bundler for a specific app, then run the Android install in a second terminal. The app will connect to Metro over USB/Wi-Fi for hot reload.

```bash
# Terminal 1 — Metro bundler
npm run dev:analyzer     # or dev:fm, dev:air, dev:ham, dev:noaa

# Terminal 2 — install to connected device or running emulator
npm run android:analyzer # or android:fm, android:air, android:ham, android:noaa
```

The `android:*` scripts call `./gradlew install<Flavor>Debug` which skips JS bundling and connects to Metro instead.

---

## Build APK

To produce a self-contained debug APK (JS bundled in, no Metro needed):

```bash
npm run build:analyzer   # or build:fm, build:air, build:ham, build:noaa
```

Or build all flavors at once:

```bash
npm run build:all
```

APKs are written to `android/app/build/outputs/apk/<flavor>/debug/`.

There is also a convenience script that builds Rust first then assembles the APK:

```bash
./scripts/build-flavor.sh analyzer   # or fm, air, ham, noaa
```

---

## Project Structure

```
sdr-go/
  apps/
    analyzer/          Spectrum analyzer app
    fm/                FM broadcast app
    air/               Airband app
    ham/               Ham radio app
    noaa/              NOAA weather app
  packages/
    ui-core/           Shared React Native components, hooks, SdrModule wrapper
  android/             Single shared Android project (all flavors)
    app/src/main/
      cpp/             CMakeLists.txt — links libsdr_core.so
      java/com/sdrgo/  Kotlin — SdrModule JNI bridge, MainApplication
      jniLibs/         Pre-built .so files (populated by build:rust)
  rust/
    sdr_core/          Rust signal processing library (see rust/sdr_core/README.md)
  scripts/
    build-rust.sh      Compile Rust for all Android ABIs
    build-flavor.sh    Build Rust + APK for one flavor end-to-end
```

---

## Architecture

```
React Native / Expo (apps/*)
        ↕
@sdrgo/ui-core  (packages/ui-core)   SdrModule TS wrapper, shared components
        ↕
Kotlin Native Module (android/)      JNI bridge, AudioTrack, USB permissions
        ↕
sdr_core (rust/)                     Signal processing, demodulators, decoders
        ↕
RTL-SDR hardware via USB OTG
```

---

## Troubleshooting

**`libsdr_core.so` not found at build time**
Run `npm run build:rust` first. The `.so` files must exist in `android/app/src/main/jniLibs/` before Gradle runs.

**Metro can't find a module**
Make sure you started Metro with the matching `dev:<flavor>` command. The `SDRGO_APP` env var tells Metro which app's entry point to resolve.

**Wrong app installs to device**
Each flavor has a distinct `applicationId` (`com.sdrgo.<flavor>`), so all five can coexist. If the wrong one appears, check which `android:*` command you ran.

**Gradle can't find `hermesc`**
Ensure `npm install` has been run from the repo root so `node_modules/hermes-compiler` is present.

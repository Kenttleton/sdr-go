set dotenv-load := false

NDK_VERSION := "27.1.12297006"
NDK_HOME := env_var('HOME') + "/Library/Android/sdk/ndk/" + NDK_VERSION

FLAVORS := "analyzer fm air ham noaa"

# List available recipes
default:
    @just --list

# ── Rust ─────────────────────────────────────────────────────────────────────

# Build sdr_core for the host
core:
    cd rust && cargo build --release -p sdr_core

# Cross-compile sdr_jni for Android (requires cargo-ndk)
jni:
    cd rust && ANDROID_NDK_HOME={{NDK_HOME}} cargo ndk \
      -t aarch64-linux-android \
      -t x86_64-linux-android \
      build --release -p sdr_jni

# Build sdr_srv for the host
srv:
    cd rust && cargo build --release -p sdr_srv

# Build all Rust crates
rust: core jni srv

# ── Android ──────────────────────────────────────────────────────────────────

# Install a debug APK for a flavor  [analyzer|fm|air|ham|noaa]
android flavor:
    #!/usr/bin/env bash
    set -e
    if [[ ! " {{FLAVORS}} " =~ " {{flavor}} " ]]; then
      echo "Unknown flavor: {{flavor}}  valid: {{FLAVORS}}" && exit 1
    fi
    cd android && ./gradlew install$(echo "{{flavor}}" | sed 's/./\u&/')Debug -PsdrgoApp={{flavor}}

# ── Full builds ───────────────────────────────────────────────────────────────

# Full build for a flavor: Rust JNI + Android APK  [analyzer|fm|air|ham|noaa]
build flavor: jni
    #!/usr/bin/env bash
    set -e
    if [[ ! " {{FLAVORS}} " =~ " {{flavor}} " ]]; then
      echo "Unknown flavor: {{flavor}}  valid: {{FLAVORS}}" && exit 1
    fi
    cd android
    ./gradlew assemble$(echo "{{flavor}}" | sed 's/./\u&/')Debug -PsdrgoApp={{flavor}}
    echo "APK → android/app/build/outputs/apk/{{flavor}}/debug/"

# ── Dev ──────────────────────────────────────────────────────────────────────

# Start Expo dev server for a flavor  [analyzer|fm|air|ham|noaa]
dev flavor:
    SDRGO_APP={{flavor}} node_modules/.bin/expo start apps/{{flavor}} --android --dev-client

# ── Quality ───────────────────────────────────────────────────────────────────

# Type-check all packages
typecheck:
    npx turbo run typecheck

# Lint all packages
lint:
    npx turbo run lint

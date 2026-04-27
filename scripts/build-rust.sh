#!/bin/bash
set -e

NDK_VERSION="27.1.12297006"
ANDROID_NDK_HOME="$HOME/Library/Android/sdk/ndk/$NDK_VERSION"
export ANDROID_NDK_HOME

echo "Building sdr_core for Android targets..."

cd "$(dirname "$0")/../rust/sdr_core"

cargo ndk \
  -t aarch64-linux-android \
  -t x86_64-linux-android \
  build --release

echo "Rust build complete."
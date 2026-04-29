#!/bin/bash
set -e

FLAVOR=${1:-analyzer}
VALID_FLAVORS=("analyzer" "fm" "air" "ham" "noaa")

# Validate flavor
if [[ ! " ${VALID_FLAVORS[@]} " =~ " ${FLAVOR} " ]]; then
  echo "❌ Unknown flavor: $FLAVOR"
  echo "   Valid: ${VALID_FLAVORS[*]}"
  exit 1
fi

echo "🦀 Building sdr_core..."
./scripts/build-rust.sh

echo "🤖 Building Android $FLAVOR APK (Gradle bundles JS via expo export:embed)..."
cd android
./gradlew assemble$(echo $FLAVOR | sed 's/./\u&/')Debug -PsdrgoApp=$FLAVOR
cd ..

echo "✅ Done! APK at android/app/build/outputs/apk/$FLAVOR/debug/"
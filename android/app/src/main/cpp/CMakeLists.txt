cmake_minimum_required(VERSION 3.22.1)
project("sdrgo")

set(CMAKE_VERBOSE_MAKEFILE ON)
set(PACKAGE_NAME "sdrgo")

# Rust pre-built library — either copied by copyRustLibs or committed to jniLibs
set(RUST_LIB_DIR "${CMAKE_SOURCE_DIR}/../jniLibs/${ANDROID_ABI}")

add_library(sdr_core SHARED IMPORTED)
set_target_properties(sdr_core PROPERTIES
    IMPORTED_LOCATION "${RUST_LIB_DIR}/libsdr_core.so"
)

# Required because externalNativeBuild replaces the React Native Gradle
# plugin's automatic CMake step. This include restores TurboModule codegen
# and core native setup (PlatformConstants, etc.).
include(${REACT_ANDROID_DIR}/cmake-utils/ReactNative-application.cmake)
cmake_minimum_required(VERSION 3.22.1)
project("sdr_core")

# Path to Rust compiled library
set(RUST_LIB_DIR
    "${CMAKE_SOURCE_DIR}/../../../../../../rust/sdr_core/target/${ANDROID_ABI}/release"
)

add_library(sdr_core SHARED IMPORTED)

set_target_properties(sdr_core PROPERTIES
    IMPORTED_LOCATION "${RUST_LIB_DIR}/libsdr_core.so"
)

target_include_directories(sdr_core INTERFACE
    "${CMAKE_SOURCE_DIR}/include"
)
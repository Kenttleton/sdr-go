//! sdr_core – pure Rust SDR signal processing library.
//!
//! Entry points:
//! - `sdr_jni`  — thin cdylib that wraps this crate for Android/JNI
//! - `sdr_srv`  — network binary that exposes this crate over HTTP/WebSocket

#![allow(dead_code)]

pub mod pipeline;
pub mod service;
pub mod usb;

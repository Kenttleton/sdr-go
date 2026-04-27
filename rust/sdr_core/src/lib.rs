use jni::JNIEnv;
use jni::objects::JClass;
use jni::sys::jstring;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_com_sdrgo_SdrModule_coreVersion(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
   let version = format!(
        "sdr_core v{} - Rust 1.92.0 - pipeline ready",
        env!("CARGO_PKG_VERSION"),
    );
    env.new_string(version)
        .expect("Failed to create string")
        .into_raw()
}
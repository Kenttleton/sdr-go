package com.sdrgo

import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import com.facebook.react.bridge.Promise

class SdrModule(reactContext: ReactApplicationContext) :
    ReactContextBaseJavaModule(reactContext) {

    override fun getName(): String = "SdrModule"

    @ReactMethod
    fun getCoreVersion(promise: Promise) {
        try {
            promise.resolve(coreVersion())
        } catch (e: Exception) {
            promise.reject("SDR_ERROR", e.message)
        }
    }

    companion object {
        init {
            System.loadLibrary("sdr_core")
        }

        @JvmStatic
        external fun coreVersion(): String
    }
}
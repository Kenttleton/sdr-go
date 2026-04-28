package com.sdrgo

import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import com.facebook.react.bridge.Promise

class SdrModule(reactContext: ReactApplicationContext) :
    ReactContextBaseJavaModule(reactContext) {

    override fun getName(): String = "SdrModule"

    private val usbPermissionManager = UsbPermissionManager(reactContext)

    init {
        usbPermissionManager.registerReceiver()
    }

    @ReactMethod
    fun getCoreVersion(promise: Promise) {
        try {
            promise.resolve(coreVersion())
        } catch (e: Exception) {
            promise.reject("SDR_ERROR", e.message)
        }
    }

    @ReactMethod
    fun requestUsbPermission(promise: Promise) {
        usbPermissionManager.requestPermission { fd ->
            if (fd != null) {
                promise.resolve(fd)
            } else {
                promise.reject("USB_ERROR", "No RTL-SDR device found or permission denied")
            }
        }
    }

    override fun invalidate() {
        usbPermissionManager.unregisterReceiver()
        super.invalidate()
    }

    companion object {
        init {
            System.loadLibrary("sdr_core")
        }

        @JvmStatic
        external fun coreVersion(): String
    }
}
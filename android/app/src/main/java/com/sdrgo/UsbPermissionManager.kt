package com.sdrgo

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbManager
import android.os.Build

class UsbPermissionManager(private val context: Context) {

    companion object {
        private const val ACTION_USB_PERMISSION =
            "com.sdrgo.USB_PERMISSION"
    }

    private val usbManager =
        context.getSystemService(Context.USB_SERVICE) as UsbManager

    // Callback invoked on permission result
    private var onPermissionResult: ((fd: Int?) -> Unit)? = null

    private val permissionReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            if (intent.action == ACTION_USB_PERMISSION) {
                val device: UsbDevice? =
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                        intent.getParcelableExtra(
                            UsbManager.EXTRA_DEVICE,
                            UsbDevice::class.java
                        )
                    } else {
                        @Suppress("DEPRECATION")
                        intent.getParcelableExtra(UsbManager.EXTRA_DEVICE)
                    }

                val granted = intent.getBooleanExtra(
                    UsbManager.EXTRA_PERMISSION_GRANTED, false
                )

                if (granted && device != null) {
                    onPermissionResult?.invoke(openDevice(device))
                } else {
                    onPermissionResult?.invoke(null)
                }
            }
        }
    }

    fun registerReceiver() {
        val filter = IntentFilter(ACTION_USB_PERMISSION)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            context.registerReceiver(
                permissionReceiver,
                filter,
                Context.RECEIVER_NOT_EXPORTED
            )
        } else {
            context.registerReceiver(permissionReceiver, filter)
        }
    }

    fun unregisterReceiver() {
        try {
            context.unregisterReceiver(permissionReceiver)
        } catch (e: IllegalArgumentException) {
            // Already unregistered
        }
    }

    /// Find first RTL-SDR device and request permission
    fun requestPermission(onResult: (fd: Int?) -> Unit) {
        onPermissionResult = onResult

        val device = findRtlSdrDevice()
        if (device == null) {
            onResult(null)
            return
        }

        if (usbManager.hasPermission(device)) {
            // Already have permission — open immediately
            onResult(openDevice(device))
            return
        }

        val flags = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_MUTABLE
        } else {
            PendingIntent.FLAG_UPDATE_CURRENT
        }

        val permissionIntent = PendingIntent.getBroadcast(
            context, 0,
            Intent(ACTION_USB_PERMISSION),
            flags
        )

        usbManager.requestPermission(device, permissionIntent)
    }

    private fun findRtlSdrDevice(): UsbDevice? {
        return usbManager.deviceList.values.firstOrNull { device ->
            isRtlSdrDevice(device)
        }
    }

    /// RTL-SDR devices share a small set of USB vendor/product IDs
    /// RTL-SDR Blog V3 is 0x0bda / 0x2838
    private fun isRtlSdrDevice(device: UsbDevice): Boolean {
        val knownDevices = listOf(
            Pair(0x0bda, 0x2832),
            Pair(0x0bda, 0x2838), // RTL-SDR Blog V3
            Pair(0x0bda, 0x2839),
            Pair(0x0bda, 0x283a),
            Pair(0x0bda, 0x283b),
            Pair(0x0bda, 0x283c),
            Pair(0x0bda, 0x283d),
            Pair(0x0bda, 0x283f),
        )
        return knownDevices.any { (vid, pid) ->
            device.vendorId == vid && device.productId == pid
        }
    }

    private fun openDevice(device: UsbDevice): Int? {
        val connection = usbManager.openDevice(device) ?: return null
        return connection.fileDescriptor
    }
}
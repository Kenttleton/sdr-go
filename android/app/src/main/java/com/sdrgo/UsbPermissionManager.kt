package com.sdrgo

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbDeviceConnection
import android.hardware.usb.UsbManager
import android.os.Build
import android.util.Log

class UsbPermissionManager(private val context: Context) {

    companion object {
        private const val ACTION_USB_PERMISSION = "com.sdrgo.USB_PERMISSION"
        private const val TAG = "SdrUsbPermission"
    }

    private val usbManager =
        context.getSystemService(Context.USB_SERVICE) as UsbManager

    // Callback invoked on permission result
    private var onPermissionResult: ((fd: Int?) -> Unit)? = null

    // Invoked when the RTL-SDR device is physically unplugged
    var onDeviceDetached: (() -> Unit)? = null

    // Held so we can release the USB interface claim on closeConnection()
    private var activeConnection: UsbDeviceConnection? = null

    private val permissionReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            when (intent.action) {
                ACTION_USB_PERMISSION -> {
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

                    Log.d(TAG, "USB permission result: granted=$granted device=${device?.deviceName}")

                    if (granted && device != null) {
                        val fd = openDevice(device)
                        Log.d(TAG, "Opened device fd=$fd")
                        onPermissionResult?.invoke(fd)
                    } else {
                        Log.w(TAG, "USB permission denied or device null")
                        onPermissionResult?.invoke(null)
                    }
                }
                UsbManager.ACTION_USB_DEVICE_DETACHED -> {
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
                    if (device != null && isRtlSdrDevice(device)) {
                        Log.i(TAG, "RTL-SDR unplugged: ${device.deviceName}")
                        onDeviceDetached?.invoke()
                    }
                }
            }
        }
    }

    fun registerReceiver() {
        val filter = IntentFilter(ACTION_USB_PERMISSION).apply {
            addAction(UsbManager.ACTION_USB_DEVICE_DETACHED)
        }
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

    /** Release the USB interface claim so other apps (or this app after a restart) can open the device. */
    fun closeConnection() {
        activeConnection?.let {
            Log.d(TAG, "Closing UsbDeviceConnection")
            it.close()
        }
        activeConnection = null
    }

    /// Find first RTL-SDR device and request permission
    fun requestPermission(onResult: (fd: Int?) -> Unit) {
        onPermissionResult = onResult

        val allDevices = usbManager.deviceList
        Log.d(TAG, "USB devices attached: ${allDevices.size}")
        allDevices.values.forEach { d ->
            Log.d(TAG, "  device vid=0x${d.vendorId.toString(16)} pid=0x${d.productId.toString(16)} name=${d.deviceName}")
        }

        val device = findRtlSdrDevice()
        if (device == null) {
            Log.w(TAG, "No RTL-SDR device found in device list")
            onResult(null)
            return
        }

        Log.d(TAG, "Found RTL-SDR: vid=0x${device.vendorId.toString(16)} pid=0x${device.productId.toString(16)}")

        if (usbManager.hasPermission(device)) {
            Log.d(TAG, "Already have USB permission, opening device")
            onResult(openDevice(device))
            return
        }

        Log.d(TAG, "Requesting USB permission (SDK=${Build.VERSION.SDK_INT})")

        val flags = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_MUTABLE
        } else {
            PendingIntent.FLAG_UPDATE_CURRENT
        }

        val permissionIntent = PendingIntent.getBroadcast(
            context, 0,
            Intent(ACTION_USB_PERMISSION).apply { `package` = context.packageName },
            flags
        )

        usbManager.requestPermission(device, permissionIntent)
        Log.d(TAG, "USB permission dialog shown")
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
        closeConnection() // release any previous claim first
        val connection = usbManager.openDevice(device) ?: return null
        activeConnection = connection
        return connection.fileDescriptor
    }
}
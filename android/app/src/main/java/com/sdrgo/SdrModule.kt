package com.sdrgo

import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import com.facebook.react.bridge.Promise
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioTrack
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.isActive

class SdrModule(reactContext: ReactApplicationContext) :
    ReactContextBaseJavaModule(reactContext) {

    override fun getName(): String = "SdrModule"

    private val usbPermissionManager = UsbPermissionManager(reactContext)
    private var audioTrack: AudioTrack? = null
    private var audioJob: Job? = null
    private val scope = CoroutineScope(Dispatchers.IO)
    private val waveformBuffer = FloatArray(512)
    @Volatile private var waveformReady = false

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
                promise.reject(
                    "USB_ERROR",
                    "No RTL-SDR device found or permission denied"
                )
            }
        }
    }

    @ReactMethod
    fun startFm(
        fd: Int,
        frequencyHz: Double,
        audioSampleRate: Int,
        stereo: Boolean,
        promise: Promise
    ) {
        try {
            val opened = openDevice(fd, frequencyHz.toLong(), audioSampleRate, stereo)
            if (!opened) {
                promise.reject("SDR_ERROR", "Failed to open device")
                return
            }

            startAudioTrack(audioSampleRate)
            startAudioLoop()
            promise.resolve(true)
        } catch (e: Exception) {
            promise.reject("SDR_ERROR", e.message)
        }
    }

    @ReactMethod
    fun tuneFrequency(frequencyHz: Double, promise: Promise) {
        try {
            val success = setFrequency(frequencyHz.toLong())
            promise.resolve(success)
        } catch (e: Exception) {
            promise.reject("SDR_ERROR", e.message)
        }
    }

    @ReactMethod
    fun stopFm(promise: Promise) {
        try {
            audioJob?.cancel()
            audioJob = null
            audioTrack?.stop()
            audioTrack?.release()
            audioTrack = null
            closeDevice()
            promise.resolve(true)
        } catch (e: Exception) {
            promise.reject("SDR_ERROR", e.message)
        }
    }

    @ReactMethod
    fun checkStereo(promise: Promise) {
        promise.resolve(isStereoDetected())
    }

    // ── Private audio helpers ────────────────────────────────────────────────

    private fun startAudioTrack(sampleRate: Int) {
        audioTrack?.release()

        val minBuffer = AudioTrack.getMinBufferSize(
            sampleRate,
            AudioFormat.CHANNEL_OUT_STEREO,
            AudioFormat.ENCODING_PCM_FLOAT
        )

        // Use 4x min buffer for stability
        val bufferSize = minBuffer * 4

        audioTrack = AudioTrack.Builder()
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(AudioAttributes.USAGE_MEDIA)
                    .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                    .build()
            )
            .setAudioFormat(
                AudioFormat.Builder()
                    .setSampleRate(sampleRate)
                    .setEncoding(AudioFormat.ENCODING_PCM_FLOAT)
                    .setChannelMask(AudioFormat.CHANNEL_OUT_STEREO)
                    .build()
            )
            .setBufferSizeInBytes(bufferSize)
            .setTransferMode(AudioTrack.MODE_STREAM)
            .build()

        audioTrack?.play()
    }

    private fun startAudioLoop() {
        audioJob?.cancel()
        audioJob = scope.launch {
            while (isActive) {
                val pcm = getAudioBuffer()
                if (pcm.isNotEmpty()) {
                    audioTrack?.write(
                        pcm, 0, pcm.size,
                        AudioTrack.WRITE_NON_BLOCKING
                    )
                    // Downsample PCM to 512 points for waveform display
                    // Take every Nth stereo frame (interleaved L/R)
                    // Use left channel only for waveform
                    val step = maxOf(1, pcm.size / (512 * 2))
                    for (i in 0 until 512) {
                        val idx = (i * step * 2).coerceAtMost(pcm.size - 2)
                        waveformBuffer[i] = pcm[idx] // left channel
                    }
                    waveformReady = true
                }
            }
        }
    }

    @ReactMethod
    fun getWaveformBuffer(promise: Promise) {
        if (!waveformReady) {
            promise.resolve(null)
            return
        }
        // Convert to WritableArray for React Native bridge
        val arr = com.facebook.react.bridge.Arguments.createArray()
        for (f in waveformBuffer) {
            arr.pushDouble(f.toDouble())
        }
        promise.resolve(arr)
    }

    override fun invalidate() {
        audioJob?.cancel()
        audioTrack?.stop()
        audioTrack?.release()
        usbPermissionManager.unregisterReceiver()
        closeDevice()
        super.invalidate()
    }

    // ── Native declarations ──────────────────────────────────────────────────

    companion object {
        init {
            System.loadLibrary("sdr_core")
        }

        @JvmStatic external fun coreVersion(): String
        @JvmStatic external fun openDevice(
            fd: Int,
            frequencyHz: Long,
            audioSampleRate: Int,
            stereo: Boolean
        ): Boolean
        @JvmStatic external fun setFrequency(frequencyHz: Long): Boolean
        @JvmStatic external fun getAudioBuffer(): FloatArray
        @JvmStatic external fun isStereoDetected(): Boolean
        @JvmStatic external fun closeDevice()
    }
}
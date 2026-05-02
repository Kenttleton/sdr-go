package com.sdrgo

import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioTrack
import android.os.Environment
import android.util.Log
import com.facebook.react.bridge.Arguments
import com.facebook.react.bridge.Promise
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import com.facebook.react.bridge.ReadableArray
import com.facebook.react.modules.core.DeviceEventManagerModule
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.io.File
import java.io.FileOutputStream
import java.io.RandomAccessFile
import java.nio.ByteBuffer
import java.nio.ByteOrder

class SdrModule(reactContext: ReactApplicationContext) :
    ReactContextBaseJavaModule(reactContext) {

    override fun getName(): String = "SdrModule"

    companion object {
        private const val TAG = "SdrModule"
        init { System.loadLibrary("sdr_core") }

        @JvmStatic external fun coreVersion(): String
        @JvmStatic external fun openDevice(
            fd: Int,
            frequencyHz: Long,
            stereo: Boolean,
            stationsMode: Boolean
        ): Boolean
        @JvmStatic external fun setFrequency(frequencyHz: Long): Boolean
        @JvmStatic external fun getAudioBuffer(): FloatArray
        @JvmStatic external fun isStereoDetected(): Boolean
        @JvmStatic external fun closeDevice()
        @JvmStatic external fun nativeGetSignalStrength(): Float
        @JvmStatic external fun nativeGetRdsInfo(): String
        @JvmStatic external fun nativeSetGain(tenthsDb: Int, autoGain: Boolean): Boolean
        @JvmStatic external fun nativeGetAvailableGains(): IntArray
        @JvmStatic external fun nativeSetEq(bands: FloatArray): Boolean
        @JvmStatic external fun nativeSetMonoMode(mono: Boolean): Boolean
        @JvmStatic external fun nativeFlushStream()
    }

    private val usbPermissionManager = UsbPermissionManager(reactContext)
    @Volatile private var audioTrack: AudioTrack? = null
    private var audioJob: Job? = null
    private var scanJob: Job? = null
    private val scope = CoroutineScope(Dispatchers.IO)

    // Waveform display — 512-point downsampled left-channel PCM
    private val waveformBuffer = FloatArray(512)
    @Volatile private var waveformReady = false

    // Recording state
    @Volatile private var isRecording = false
    private val recordingChunks = mutableListOf<FloatArray>()
    private var recordingPath: String? = null
    private var recordingAudioRate: Int = 96_000

    // Current audio sample rate — needed for WAV header and AudioTrack
    private var currentAudioRate: Int = 96_000

    // Scan state
    @Volatile private var scanActive = false

    init {
        usbPermissionManager.registerReceiver()
        usbPermissionManager.onDeviceDetached = { handleUsbDetached() }
    }

    // ── USB permission ─────────────────────────────────────────────────────────

    @ReactMethod
    fun getCoreVersion(promise: Promise) {
        try { promise.resolve(coreVersion()) }
        catch (e: Exception) { promise.reject("SDR_ERROR", e.message) }
    }

    @ReactMethod
    fun requestUsbPermission(promise: Promise) {
        try {
            usbPermissionManager.requestPermission { fd ->
                if (fd != null) promise.resolve(fd)
                else promise.reject("USB_ERROR", "No RTL-SDR device found or permission denied")
            }
        } catch (e: Exception) {
            promise.reject("USB_ERROR", "USB permission request failed: ${e.message}")
        }
    }

    // ── Start / stop ───────────────────────────────────────────────────────────

    /**
     * Open device and begin audio playback.
     * stationsMode = true  → 2.4 MSPS, 240 kHz intermediate, 48 kHz audio, RDS decoder active
     * stationsMode = false → 2.048 MSPS, single-stage, 96 kHz audio (FM Wide)
     */
    @ReactMethod
    fun startFm(
        fd: Int,
        frequencyHz: Double,
        stereo: Boolean,
        stationsMode: Boolean,
        promise: Promise
    ) {
        try {
            currentAudioRate = if (stationsMode) 48_000 else 96_000
            emitLog("log", "startFm: fd=$fd freq=${frequencyHz.toLong()} stereo=$stereo stations=$stationsMode audioRate=$currentAudioRate")
            val opened = openDevice(fd, frequencyHz.toLong(), stereo, stationsMode)
            if (!opened) {
                emitLog("error", "startFm: openDevice returned false")
                promise.reject("SDR_ERROR", "Failed to open device")
                return
            }
            emitLog("log", "startFm: device opened OK")
            startAudioTrack(currentAudioRate)
            startAudioLoop()
            promise.resolve(true)
        } catch (e: Exception) {
            emitLog("error", "startFm exception: ${e.message}")
            promise.reject("SDR_ERROR", e.message)
        }
    }

    @ReactMethod
    fun tuneFrequency(frequencyHz: Double, promise: Promise) {
        try {
            promise.resolve(setFrequency(frequencyHz.toLong()))
        } catch (e: Exception) {
            promise.reject("SDR_ERROR", e.message)
        }
    }

    @ReactMethod
    fun stopFm(promise: Promise) {
        try {
            stopPlayback()
            promise.resolve(true)
        } catch (e: Exception) {
            promise.reject("SDR_ERROR", e.message)
        }
    }

    private fun stopPlayback() {
        scanJob?.cancel()
        scanJob    = null
        scanActive = false
        audioJob?.cancel()
        audioJob   = null
        // Null the track reference BEFORE releasing so the audio loop's
        // volatile read sees null and skips any in-flight write.
        val track = audioTrack
        audioTrack = null
        track?.stop()
        track?.release()
        closeDevice()
        usbPermissionManager.closeConnection()
    }

    private fun handleUsbDetached() {
        emitLog("warn", "USB device unplugged — stopping playback")
        try {
            stopPlayback()
        } catch (e: Exception) {
            emitLog("error", "handleUsbDetached: ${e.message}")
        }
        try {
            emitEvent("onUsbDeviceDetached", Arguments.createMap())
        } catch (_: Exception) {
            // Bridge may not be ready (e.g. activity is being destroyed)
        }
    }

    // ── Waveform + stereo readouts ─────────────────────────────────────────────

    @ReactMethod
    fun checkStereo(promise: Promise) {
        promise.resolve(isStereoDetected())
    }

    @ReactMethod
    fun getWaveformBuffer(promise: Promise) {
        if (!waveformReady) { promise.resolve(null); return }
        val arr = Arguments.createArray()
        for (f in waveformBuffer) arr.pushDouble(f.toDouble())
        promise.resolve(arr)
    }

    // ── Signal strength ────────────────────────────────────────────────────────

    @ReactMethod
    fun getSignalStrength(promise: Promise) {
        promise.resolve(nativeGetSignalStrength().toDouble())
    }

    // ── RDS data ───────────────────────────────────────────────────────────────

    @ReactMethod
    fun getRdsInfo(promise: Promise) {
        promise.resolve(nativeGetRdsInfo())
    }

    // ── Hardware gain ──────────────────────────────────────────────────────────

    @ReactMethod
    fun getAvailableGains(promise: Promise) {
        val gains = nativeGetAvailableGains()
        val arr = Arguments.createArray()
        for (g in gains) arr.pushInt(g)
        promise.resolve(arr)
    }

    @ReactMethod
    fun setGain(tenthsDb: Int, autoGain: Boolean, promise: Promise) {
        try {
            promise.resolve(nativeSetGain(tenthsDb, autoGain))
        } catch (e: Exception) {
            promise.reject("SDR_ERROR", e.message)
        }
    }

    // ── EQ ─────────────────────────────────────────────────────────────────────

    @ReactMethod
    fun setEq(bands: ReadableArray, promise: Promise) {
        try {
            val floats = FloatArray(bands.size()) { bands.getDouble(it).toFloat() }
            promise.resolve(nativeSetEq(floats))
        } catch (e: Exception) {
            promise.reject("SDR_ERROR", e.message)
        }
    }

    // ── Mono mode ──────────────────────────────────────────────────────────────

    @ReactMethod
    fun setMonoMode(mono: Boolean, promise: Promise) {
        try {
            promise.resolve(nativeSetMonoMode(mono))
        } catch (e: Exception) {
            promise.reject("SDR_ERROR", e.message)
        }
    }

    // ── Scan ───────────────────────────────────────────────────────────────────
    //
    // The RTL-SDR is a single receiver — the audio from the current station is
    // muted during scan (volume → 0). The UI receives progress events and a final
    // onScanComplete / onScanFailed event, then audio resumes on the new station.
    //
    // Steps per scan: 100 kHz for FM, 10 kHz for AM.
    // After each tune we wait 150 ms for the tuner to settle and fill the ring
    // buffer, then read signal strength.

    @ReactMethod
    fun scan(
        startFreqHz: Double,
        direction: String,
        band: String,
        thresholdDb: Double,
        promise: Promise
    ) {
        if (scanActive) {
            promise.reject("SCAN_BUSY", "Scan already in progress")
            return
        }

        val step = if (band == "fm") 100_000L else 10_000L
        val minHz = if (band == "fm") 87_500_000L else 520_000L
        val maxHz = if (band == "fm") 108_000_000L else 1_710_000L
        val signalThreshold = (thresholdDb / 100.0).toFloat().coerceIn(0.02f, 0.9f)

        scanActive = true
        audioTrack?.setVolume(0f) // mute during scan

        scanJob = scope.launch {
            var freq = startFreqHz.toLong()
            var found = false

            repeat(((maxHz - minHz) / step + 1).toInt()) {
                if (!isActive || !scanActive) return@repeat

                freq = if (direction == "up") {
                    if (freq + step > maxHz) minHz else freq + step
                } else {
                    if (freq - step < minHz) maxHz else freq - step
                }

                setFrequency(freq)
                delay(150) // let tuner settle + fill ring buffer

                // Drain a small IQ block to update signal_power
                getAudioBuffer()

                val strength = nativeGetSignalStrength()
                emitEvent("onScanProgress", Arguments.createMap().apply {
                    putDouble("frequencyHz", freq.toDouble())
                    putDouble("strength", strength.toDouble())
                })

                if (strength >= signalThreshold) {
                    found = true
                    return@repeat
                }
            }

            audioTrack?.setVolume(1f) // unmute

            if (found) {
                emitEvent("onScanComplete", Arguments.createMap().apply {
                    putDouble("frequencyHz", freq.toDouble())
                })
            } else {
                // Restore original frequency
                setFrequency(startFreqHz.toLong())
                emitEvent("onScanFailed", Arguments.createMap())
            }
            scanActive = false
        }

        promise.resolve(true)
    }

    @ReactMethod
    fun cancelScan(promise: Promise) {
        scanJob?.cancel()
        scanJob   = null
        scanActive = false
        audioTrack?.setVolume(1f)
        promise.resolve(true)
    }

    // ── Recording ──────────────────────────────────────────────────────────────
    //
    // PCM floats are accumulated in memory during the audio loop, then flushed
    // to a 16-bit PCM WAV file on stopRecording(). Files land in the app's
    // external files directory (no WRITE_EXTERNAL_STORAGE permission needed on
    // Android 10+), which the system file-picker can access via FileProvider.

    @ReactMethod
    fun startRecording(filename: String, promise: Promise) {
        if (isRecording) {
            promise.reject("REC_ERROR", "Already recording")
            return
        }
        synchronized(recordingChunks) { recordingChunks.clear() }
        recordingPath     = filename
        recordingAudioRate = currentAudioRate
        isRecording = true
        promise.resolve(true)
    }

    @ReactMethod
    fun stopRecording(promise: Promise) {
        if (!isRecording) {
            promise.reject("REC_ERROR", "Not recording")
            return
        }
        isRecording = false

        scope.launch {
            try {
                val path = saveWav()
                promise.resolve(path)
            } catch (e: Exception) {
                promise.reject("REC_ERROR", e.message)
            }
        }
    }

    // ── Private audio helpers ──────────────────────────────────────────────────

    private fun startAudioTrack(sampleRate: Int) {
        audioTrack?.release()
        val minBuf = AudioTrack.getMinBufferSize(
            sampleRate,
            AudioFormat.CHANNEL_OUT_STEREO,
            AudioFormat.ENCODING_PCM_FLOAT
        )
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
            .setBufferSizeInBytes(minBuf * 8)
            .setTransferMode(AudioTrack.MODE_STREAM)
            .build()
        audioTrack?.play()
    }

    private fun startAudioLoop() {
        audioJob?.cancel()
        // Channel decouples the USB fill (producer) from the AudioTrack write (consumer).
        // Without this, a single coroutine does fill→write sequentially: 17ms USB read
        // followed by 17ms of audio leaves zero slack for OS jitter → underruns/crackle.
        // With the channel, the consumer's WRITE_BLOCKING overlaps the producer's USB fill.
        val pcmChannel = Channel<FloatArray>(capacity = 8)

        audioJob = scope.launch {
            // Producer: calls getAudioBuffer() (USB fill + DSP) and hands chunks to consumer.
            val producerJob = launch {
                var frameCount = 0L
                var emptyCount = 0
                while (isActive) {
                    val pcm = getAudioBuffer()
                    if (pcm.isEmpty()) {
                        emptyCount++
                        if (emptyCount == 1 || emptyCount % 50 == 0) {
                            emitLog("warn", "audioLoop: empty PCM #$emptyCount")
                        }
                        // Fallback: if the USB detach broadcast hasn't fired yet,
                        // treat sustained fill failures as device loss and stop.
                        if (emptyCount >= 60) {
                            emitLog("warn", "audioLoop: device appears lost after $emptyCount empty buffers — stopping")
                            handleUsbDetached()
                            break
                        }
                        continue
                    }
                    emptyCount = 0
                    frameCount++
                    if (frameCount == 1L) {
                        emitLog("log", "audioLoop: first chunk — ${pcm.size} floats")
                    } else if (frameCount % 500L == 0L) {
                        emitLog("log", "audioLoop: frame #$frameCount pcm=${pcm.size} signal=${
                            String.format("%.4f", nativeGetSignalStrength())}")
                    }

                    // Side-effects on the PCM chunk before handing it off
                    if (isRecording) {
                        val mono = FloatArray(pcm.size / 2) { i -> pcm[i * 2] }
                        synchronized(recordingChunks) { recordingChunks.add(mono) }
                    }
                    val step = maxOf(1, pcm.size / (512 * 2))
                    for (i in 0 until 512) {
                        waveformBuffer[i] = pcm[(i * step * 2).coerceAtMost(pcm.size - 2)]
                    }
                    waveformReady = true

                    pcmChannel.send(pcm)  // suspends if consumer is behind (back-pressure)
                }
                pcmChannel.close()
            }

            // Consumer: blocks on AudioTrack write — this overlaps with the producer's USB fill.
            for (pcm in pcmChannel) {
                try {
                    audioTrack?.write(pcm, 0, pcm.size, AudioTrack.WRITE_BLOCKING)
                } catch (_: IllegalStateException) {
                    break  // AudioTrack released by stopFm()
                }
            }
            producerJob.cancel()
        }
    }

    // ── WAV file writer ────────────────────────────────────────────────────────

    private fun saveWav(): String {
        val chunks: List<FloatArray>
        synchronized(recordingChunks) {
            chunks = recordingChunks.toList()
            recordingChunks.clear()
        }

        val dir = File(
            reactApplicationContext.getExternalFilesDir(null),
            "recordings"
        ).also { it.mkdirs() }

        val filename = recordingPath
            ?.let { if (it.endsWith(".wav")) it else "$it.wav" }
            ?: "recording_${System.currentTimeMillis()}.wav"

        val file = File(dir, filename)
        val sampleRate   = recordingAudioRate
        val numChannels  = 1
        val bitsPerSample = 16
        val totalSamples = chunks.sumOf { it.size }
        val dataBytes    = totalSamples * numChannels * bitsPerSample / 8

        FileOutputStream(file).use { fos ->
            val header = ByteBuffer.allocate(44).order(ByteOrder.LITTLE_ENDIAN)
            header.put("RIFF".toByteArray())
            header.putInt(36 + dataBytes)
            header.put("WAVE".toByteArray())
            header.put("fmt ".toByteArray())
            header.putInt(16)            // PCM sub-chunk size
            header.putShort(1)           // PCM format
            header.putShort(numChannels.toShort())
            header.putInt(sampleRate)
            header.putInt(sampleRate * numChannels * bitsPerSample / 8) // byte rate
            header.putShort((numChannels * bitsPerSample / 8).toShort()) // block align
            header.putShort(bitsPerSample.toShort())
            header.put("data".toByteArray())
            header.putInt(dataBytes)
            fos.write(header.array())

            val buf = ByteBuffer.allocate(8192 * 2).order(ByteOrder.LITTLE_ENDIAN)
            for (chunk in chunks) {
                for (sample in chunk) {
                    if (!buf.hasRemaining()) {
                        fos.write(buf.array(), 0, buf.position())
                        buf.clear()
                    }
                    val pcm16 = (sample.coerceIn(-1f, 1f) * 32767).toInt().toShort()
                    buf.putShort(pcm16)
                }
            }
            if (buf.position() > 0) fos.write(buf.array(), 0, buf.position())
        }

        return file.absolutePath
    }

    // ── React Native EventEmitter required stubs ───────────────────────────────

    @ReactMethod fun addListener(eventName: String) {}
    @ReactMethod fun removeListeners(count: Int) {}

    // ── Event emitter ──────────────────────────────────────────────────────────

    /** Emit a log entry to the JS in-app log viewer (only when the bridge is ready). */
    private fun emitLog(level: String, message: String) {
        when (level) {
            "error" -> Log.e(TAG, message)
            "warn"  -> Log.w(TAG, message)
            else    -> Log.d(TAG, message)
        }
        try {
            emitEvent("onSdrLog", Arguments.createMap().apply {
                putString("level", level)
                putString("message", "[native] $message")
            })
        } catch (_: Exception) {
            // Bridge may not be ready yet during init; logcat already captured it above
        }
    }

    private fun emitEvent(name: String, params: com.facebook.react.bridge.WritableMap) {
        reactApplicationContext
            .getJSModule(DeviceEventManagerModule.RCTDeviceEventEmitter::class.java)
            .emit(name, params)
    }

    // ── Lifecycle ──────────────────────────────────────────────────────────────

    override fun invalidate() {
        try { stopPlayback() } catch (_: Exception) {}
        usbPermissionManager.unregisterReceiver()
        super.invalidate()
    }

}

import { NativeModules, NativeEventEmitter, Platform } from 'react-native';

// Non-null at load time means the native bridge is missing.
export const driverError: string | null = NativeModules.SdrModule
  ? null
  : 'SdrModule native module not found. Ensure the Android build includes sdr_core and SdrPackage is registered.';

const getNative = () => {
  if (!NativeModules.SdrModule) throw new Error(driverError!);
  return NativeModules.SdrModule;
};

const guardAndroid = <T>(fn: () => T): T => {
  if (Platform.OS !== 'android') throw new Error('SdrGo requires Android');
  return fn();
};

// ── Event emitter ──────────────────────────────────────────────────────────────
// Wrap once so callers can subscribe to scan progress events.

export const SdrEventEmitter = NativeModules.SdrModule
  ? new NativeEventEmitter(NativeModules.SdrModule)
  : null;

// ── Module API ─────────────────────────────────────────────────────────────────

export default {
  getCoreVersion: (): Promise<string> =>
    guardAndroid(() => getNative().getCoreVersion()),

  requestUsbPermission: (): Promise<number> =>
    guardAndroid(() => getNative().requestUsbPermission()),

  /**
   * Open the RTL-SDR and start audio playback. Always opens in WFM mode.
   * Use setMode() after opening to switch to NFM or AM.
   *
   * @param fd            USB file descriptor from requestUsbPermission()
   * @param frequencyHz   Initial centre frequency in Hz
   * @param stereo        Enable stereo decode for WFM (pilot detection still required)
   * @param highQuality   true → 96 kHz audio output; false → 48 kHz (lower CPU, future RDS)
   */
  startFm: (
    fd: number,
    frequencyHz: number,
    stereo: boolean = true,
    highQuality: boolean = true,
  ): Promise<boolean> =>
    guardAndroid(() => getNative().startFm(fd, frequencyHz, stereo, highQuality)),

  tuneFrequency: (frequencyHz: number): Promise<boolean> =>
    guardAndroid(() => getNative().tuneFrequency(frequencyHz)),

  stopFm: (): Promise<boolean> =>
    guardAndroid(() => getNative().stopFm()),

  checkStereo: (): Promise<boolean> =>
    guardAndroid(() => getNative().checkStereo()),

  getWaveformBuffer: (): Promise<number[] | null> =>
    guardAndroid(() => getNative().getWaveformBuffer()),

  // ── Mode ────────────────────────────────────────────────────────────────────
  // 0 = WFM, 1 = NFM, 2 = AM-DSB, 3 = AM-USB, 4 = AM-LSB
  // Transitions between modes are crossfaded inside sdr_core.

  setMode: (mode: number): Promise<boolean> =>
    guardAndroid(() => getNative().setMode(mode)),

  // ── AM bandwidth ────────────────────────────────────────────────────────────
  // Sets the AM audio IF filter cutoff in Hz. No-op when not in an AM mode.
  // Typical values: 8000 (wide/music), 5000 (standard), 3000 (narrow), 2500 (SSB voice).

  setAmBandwidth: (bandwidthHz: number): Promise<boolean> =>
    guardAndroid(() => getNative().setAmBandwidth(bandwidthHz)),

  // ── Signal ──────────────────────────────────────────────────────────────────

  getSignalStrength: (): Promise<number> =>
    guardAndroid(() => getNative().getSignalStrength()),

  // ── Display outputs ─────────────────────────────────────────────────────────
  // All three are updated opportunistically inside getAudioBuffer() on the Kotlin
  // audio loop — poll at display rate, not audio rate.

  getIqWaveform: (): Promise<number[]> =>
    guardAndroid(() => getNative().getIqWaveform()),

  getAudioWaveform: (): Promise<number[]> =>
    guardAndroid(() => getNative().getAudioWaveform()),

  getSpectrum: (): Promise<number[]> =>
    guardAndroid(() => getNative().getSpectrum()),

  // ── RDS ─────────────────────────────────────────────────────────────────────
  // Only populated in WFM when an RDS subcarrier is present.

  getRdsInfo: (): Promise<string> =>
    guardAndroid(() => getNative().getRdsInfo()),

  // ── Hardware gain ───────────────────────────────────────────────────────────

  getAvailableGains: (): Promise<number[]> =>
    guardAndroid(() => getNative().getAvailableGains()),

  setGain: (tenthsDb: number, autoGain: boolean = false): Promise<boolean> =>
    guardAndroid(() => getNative().setGain(tenthsDb, autoGain)),

  // ── EQ ──────────────────────────────────────────────────────────────────────
  // Placeholder — parametric EQ not yet implemented in sdr_core. Always returns false.

  setEq: (bands: number[]): Promise<boolean> =>
    guardAndroid(() => getNative().setEq(bands)),

  // ── Mono mode ───────────────────────────────────────────────────────────────
  // Placeholder — stereo is auto-detected from the WFM pilot tone; there is no
  // manual mono override in the current pipeline. Always returns false.

  setMonoMode: (mono: boolean): Promise<boolean> =>
    guardAndroid(() => getNative().setMonoMode(mono)),

  // ── Scan ────────────────────────────────────────────────────────────────────
  // Non-blocking — resolves immediately. Listen to SdrEventEmitter for:
  //   onScanProgress  { frequencyHz: number; strength: number }
  //   onScanComplete  { frequencyHz: number }
  //   onScanFailed    {}

  scan: (
    currentHz: number,
    direction: 'up' | 'down',
    band: 'fm' | 'am',
    thresholdDb: number = 2,
  ): Promise<boolean> =>
    guardAndroid(() => getNative().scan(currentHz, direction, band, thresholdDb)),

  cancelScan: (): Promise<boolean> =>
    guardAndroid(() => getNative().cancelScan()),

  // ── Recording ───────────────────────────────────────────────────────────────

  startRecording: (filename: string): Promise<boolean> =>
    guardAndroid(() => getNative().startRecording(filename)),

  stopRecording: (): Promise<string> =>
    guardAndroid(() => getNative().stopRecording()),
};

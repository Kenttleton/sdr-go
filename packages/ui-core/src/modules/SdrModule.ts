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
   * Open the RTL-SDR and start audio playback.
   *
   * @param fd            USB file descriptor from requestUsbPermission()
   * @param frequencyHz   Initial centre frequency in Hz
   * @param stereo        Request stereo decode (falls back to mono when pilot absent)
   * @param stationsMode  true → FM Stations: 2.4 MSPS, 240 kHz intermediate, 48 kHz audio, RDS
   *                      false → FM Wide:     2.048 MSPS, 96 kHz audio, max audio quality
   */
  startFm: (
    fd: number,
    frequencyHz: number,
    stereo: boolean = true,
    stationsMode: boolean = false,
  ): Promise<boolean> =>
    guardAndroid(() => getNative().startFm(fd, frequencyHz, stereo, stationsMode)),

  tuneFrequency: (frequencyHz: number): Promise<boolean> =>
    guardAndroid(() => getNative().tuneFrequency(frequencyHz)),

  stopFm: (): Promise<boolean> =>
    guardAndroid(() => getNative().stopFm()),

  checkStereo: (): Promise<boolean> =>
    guardAndroid(() => getNative().checkStereo()),

  getWaveformBuffer: (): Promise<number[] | null> =>
    guardAndroid(() => getNative().getWaveformBuffer()),

  // ── Signal ──────────────────────────────────────────────────────────────────

  getSignalStrength: (): Promise<number> =>
    guardAndroid(() => getNative().getSignalStrength()),

  // ── RDS ─────────────────────────────────────────────────────────────────────
  // Only populated in stationsMode. Returns JSON string parsed by RdsModule.

  getRdsInfo: (): Promise<string> =>
    guardAndroid(() => getNative().getRdsInfo()),

  // ── Hardware gain ───────────────────────────────────────────────────────────

  getAvailableGains: (): Promise<number[]> =>
    guardAndroid(() => getNative().getAvailableGains()),

  setGain: (tenthsDb: number, autoGain: boolean = false): Promise<boolean> =>
    guardAndroid(() => getNative().setGain(tenthsDb, autoGain)),

  // ── EQ ──────────────────────────────────────────────────────────────────────

  setEq: (bands: number[]): Promise<boolean> =>
    guardAndroid(() => getNative().setEq(bands)),

  // ── Mono mode ───────────────────────────────────────────────────────────────

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

/**
 * RdsModule.ts
 *
 * Live wrappers around the native SdrModule bridge for all signal and metadata
 * calls. Functions that require the device open (RDS, signal strength, gain,
 * EQ, mono, scan, recording) silently return safe defaults when the native
 * module is unavailable — this keeps the UI functional in Expo Go / simulators.
 */

import SdrModule from './SdrModule';

// ── Types ─────────────────────────────────────────────────────────────────────

export interface RdsStationInfo {
  /** Programme Service Name — 8 char station name, e.g. "KQED-FM" */
  ps: string | null;
  /** RadioText — up to 64 chars of scrolling text */
  rt: string | null;
  /** Programme Type code 0-31 */
  pty: number | null;
  /** Programme Type Name — human-readable genre */
  ptyName: string | null;
  /** Traffic Programme flag */
  tp: boolean;
  /** Traffic Announcement flag */
  ta: boolean;
  /** Music/Speech flag: true = music */
  ms: boolean;
  /** Alternative Frequencies (populated in group 1A — not yet decoded) */
  af: number[];
  /** Clock Time if broadcast (group 4A — not yet decoded) */
  ct: string | null;
}

export interface SignalInfo {
  /** Signal strength 0.0–1.0 derived from IQ power */
  strength: number;
  /** Estimated SNR in dB (strength → dB mapping) */
  snr: number;
  /** True when 19 kHz stereo pilot is detected */
  stereo: boolean;
  /** Pilot amplitude from the stereo PLL */
  pilotAmplitude: number;
}

export interface HardwareGainInfo {
  /** Current gain in tenths of dB (e.g. 280 = 28.0 dB) */
  gainTenthsDb: number;
  /** Available gain steps in tenths of dB */
  availableGains: number[];
  /** True when in automatic gain mode */
  autoGain: boolean;
}

// ── PTY lookup ────────────────────────────────────────────────────────────────

const PTY_NAMES: Record<number, string> = {
  0:  'None',      1:  'News',         2:  'Current Affairs', 3:  'Information',
  4:  'Sport',     5:  'Education',    6:  'Drama',           7:  'Cultures',
  8:  'Science',   9:  'Varied Speech',10: 'Pop Music',
  11: 'Rock Music',12: 'Easy Listening',13:'Light Classics',
  14: 'Serious Classics', 15: 'Other Music', 16: 'Weather',
  17: 'Finance',  18: 'Childrens',   19: 'Social Affairs',
  20: 'Religion', 21: 'Phone In',    22: 'Travel',  23: 'Leisure',
  24: 'Jazz Music',25: 'Country Music',26:'National Music',
  27: 'Oldies Music',28:'Folk Music',29: 'Documentary',
  30: 'Alarm Test',31: 'Alarm',
};

export function ptyName(code: number): string {
  return PTY_NAMES[code] ?? 'Unknown';
}

// ── Internal RDS JSON type from Rust ──────────────────────────────────────────

interface RawRdsJson {
  pi: number;
  ps: string;
  rt: string;
  pty: number;
  tp: boolean;
  ta: boolean;
  ms: boolean;
  psReady: boolean;
  rtReady: boolean;
}

// ── RDS station info ──────────────────────────────────────────────────────────

/**
 * Returns live RDS metadata decoded from the hardware.
 * Only available when startFm() was called with stationsMode = true.
 * Returns null if no RDS data has been received yet.
 */
export async function getRdsStationInfo(
  _frequencyHz: number,
  band: 'fm' | 'am' = 'fm',
): Promise<RdsStationInfo | null> {
  if (band === 'am') return null;

  try {
    const json = await SdrModule.getRdsInfo();
    if (!json || json === '{}') return null;

    const raw: RawRdsJson = JSON.parse(json);
    if (!raw.psReady) return null;

    return {
      ps:      raw.ps.trim() || null,
      rt:      raw.rtReady ? (raw.rt.trim() || null) : null,
      pty:     raw.pty,
      ptyName: ptyName(raw.pty),
      tp:      raw.tp,
      ta:      raw.ta,
      ms:      raw.ms,
      af:      [],
      ct:      null,
    };
  } catch {
    return null;
  }
}

// ── Signal info ───────────────────────────────────────────────────────────────

/**
 * Returns real-time signal info from the hardware.
 * Falls back to a zero-signal result if the device is not open.
 */
export async function getSignalInfo(
  _frequencyHz: number,
  _band: 'fm' | 'am' = 'fm',
): Promise<SignalInfo> {
  try {
    const strength = await SdrModule.getSignalStrength();
    const stereo   = await SdrModule.checkStereo();

    // Map IQ power [0,1] to an approximate SNR in dB (log scale)
    const snr = strength > 0.01
      ? Math.round(20 * Math.log10(strength / 0.01) * 10) / 10
      : 0;

    return {
      strength,
      snr,
      stereo,
      pilotAmplitude: stereo ? strength * 0.3 : 0,
    };
  } catch {
    return { strength: 0, snr: 0, stereo: false, pilotAmplitude: 0 };
  }
}

// ── Hardware gain ─────────────────────────────────────────────────────────────

// Cache so we only query the device once per session
let _gainCache: HardwareGainInfo | null = null;

/**
 * Returns current gain state and available gain steps from the hardware.
 * Falls back to the RTL-SDR Blog V3 table when the device is not open.
 */
export async function getHardwareGainInfo(): Promise<HardwareGainInfo> {
  if (_gainCache) return _gainCache;

  try {
    const gains = await SdrModule.getAvailableGains();
    _gainCache = {
      gainTenthsDb:    280, // 28.0 dB — reasonable default
      availableGains:  gains.length > 0 ? gains : RTL_SDR_FALLBACK_GAINS,
      autoGain:        false,
    };
    return _gainCache;
  } catch {
    return {
      gainTenthsDb:   280,
      availableGains: RTL_SDR_FALLBACK_GAINS,
      autoGain:       false,
    };
  }
}

export async function setHardwareGain(
  tenthsDb: number,
  autoGain: boolean = false,
): Promise<boolean> {
  if (_gainCache) {
    _gainCache = { ..._gainCache, gainTenthsDb: tenthsDb, autoGain };
  }
  try {
    return await SdrModule.setGain(tenthsDb, autoGain);
  } catch {
    return false;
  }
}

// ── EQ ────────────────────────────────────────────────────────────────────────

export async function setEq(bands: number[]): Promise<boolean> {
  try {
    return await SdrModule.setEq(bands);
  } catch {
    return false;
  }
}

// ── Mono mode ─────────────────────────────────────────────────────────────────

export async function setMonoMode(mono: boolean): Promise<boolean> {
  try {
    return await SdrModule.setMonoMode(mono);
  } catch {
    return false;
  }
}

// ── Scan ──────────────────────────────────────────────────────────────────────
//
// The scan is handled by the native layer with DeviceEventEmitter events.
// This thin wrapper just starts the scan — callers should subscribe to
// SdrEventEmitter from SdrModule.ts for:
//   onScanProgress  { frequencyHz: number; strength: number }
//   onScanComplete  { frequencyHz: number }
//   onScanFailed    {}

export async function scan(
  currentHz: number,
  direction: 'up' | 'down',
  band: 'fm' | 'am',
): Promise<boolean> {
  try {
    return await SdrModule.scan(currentHz, direction, band, 2);
  } catch {
    return false;
  }
}

// ── Recording ─────────────────────────────────────────────────────────────────

export async function startRecording(filename: string): Promise<boolean> {
  try {
    return await SdrModule.startRecording(filename);
  } catch {
    return false;
  }
}

export async function stopRecording(): Promise<string> {
  try {
    return await SdrModule.stopRecording();
  } catch {
    return '';
  }
}

// ── Constants ─────────────────────────────────────────────────────────────────

export const EQ_BANDS = [
  { label: 'SUB',  freq: '60Hz',   index: 0 },
  { label: 'BASS', freq: '250Hz',  index: 1 },
  { label: 'MUD',  freq: '500Hz',  index: 2 },
  { label: 'MID',  freq: '1kHz',   index: 3 },
  { label: 'EDGE', freq: '2kHz',   index: 4 },
  { label: 'PRES', freq: '10kHz',  index: 5 },
  { label: 'AIR',  freq: '16kHz',  index: 6 },
] as const;

export const DEFAULT_EQ: number[]  = [0, 0, 0, 0, 0, 0, 0];
export const DEFAULT_GAIN_TENTHS   = 280; // 28.0 dB

// RTL-SDR Blog V3 (R820T2) fall-back gain table — matches hardware exactly
const RTL_SDR_FALLBACK_GAINS = [
  0, 9, 14, 27, 37, 77, 87, 125, 144, 157, 166, 197,
  207, 229, 254, 280, 297, 328, 338, 364, 372, 386,
  402, 421, 434, 439, 445, 480, 496,
];

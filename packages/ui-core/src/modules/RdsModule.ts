/**
 * RdsModule.ts
 *
 * Mock RDS (Radio Data System) module.
 * All functions return mock data today.
 * When the Kotlin RdsNativeModule and Rust RDS decoder are complete,
 * replace the mock implementations with NativeModules.RdsModule calls.
 *
 * Data flow: Rust RDS decoder → Kotlin RdsNativeModule → RdsModule.ts → UI
 */

import { NativeModules } from 'react-native';

// ── Types ─────────────────────────────────────────────────────────────────────

export interface RdsStationInfo {
  /** Programme Service Name — 8 char station name, e.g. "KQED-FM " */
  ps: string | null;
  /** RadioText — up to 64 chars of scrolling text */
  rt: string | null;
  /** Programme Type — genre code 0-31 */
  pty: number | null;
  /** Programme Type Name — human-readable genre */
  ptyName: string | null;
  /** Traffic Programme flag */
  tp: boolean;
  /** Traffic Announcement flag */
  ta: boolean;
  /** Music/Speech flag: true = music */
  ms: boolean;
  /** Alternative Frequencies list */
  af: number[];
  /** Clock Time if broadcast by station */
  ct: string | null;
}

export interface SignalInfo {
  /** Signal strength 0.0–1.0 */
  strength: number;
  /** SNR in dB */
  snr: number;
  /** True if 19kHz stereo pilot detected */
  stereo: boolean;
  /** Pilot amplitude 0.0–1.0 */
  pilotAmplitude: number;
}

export interface HardwareGainInfo {
  /** Current gain in tenths of dB (e.g. 496 = 49.6 dB) */
  gainTenthsDb: number;
  /** Available gain values in tenths of dB */
  availableGains: number[];
  /** True if in auto-gain mode */
  autoGain: boolean;
}

// ── PTY name lookup ───────────────────────────────────────────────────────────

const PTY_NAMES: Record<number, string> = {
  0:  'None', 1: 'News', 2: 'Current Affairs', 3: 'Information',
  4:  'Sport', 5: 'Education', 6: 'Drama', 7: 'Cultures',
  8:  'Science', 9: 'Varied Speech', 10: 'Pop Music',
  11: 'Rock Music', 12: 'Easy Listening', 13: 'Light Classics',
  14: 'Serious Classics', 15: 'Other Music', 16: 'Weather',
  17: 'Finance', 18: 'Childrens', 19: 'Social Affairs',
  20: 'Religion', 21: 'Phone In', 22: 'Travel', 23: 'Leisure',
  24: 'Jazz Music', 25: 'Country Music', 26: 'National Music',
  27: 'Oldies Music', 28: 'Folk Music', 29: 'Documentary',
  30: 'Alarm Test', 31: 'Alarm',
};

export function ptyName(code: number): string {
  return PTY_NAMES[code] ?? 'Unknown';
}

// ── Mock data pool ────────────────────────────────────────────────────────────

const MOCK_STATIONS: Record<number, Partial<RdsStationInfo>> = {
  88.5:  { ps: 'KQED-FM ', rt: 'Forum with Michael Krasny — Bay Area Public Radio', pty: 1, ptyName: 'News', ms: false },
  91.1:  { ps: 'KCSM    ', rt: 'Jazz 91 — Now Playing: Miles Davis — So What', pty: 24, ptyName: 'Jazz Music', ms: true },
  94.9:  { ps: 'KYLD    ', rt: 'Wild 94.9 — San Francisco\'s #1 Hit Music Station', pty: 10, ptyName: 'Pop Music', ms: true },
  97.3:  { ps: 'KLLC    ', rt: 'Alice 97.3 — More Variety', pty: 10, ptyName: 'Pop Music', ms: true },
  100.3: { ps: 'KBRG    ', rt: 'Bridge 100.3 — Classic Hits', pty: 12, ptyName: 'Easy Listening', ms: true },
  101.3: { ps: 'KIOI    ', rt: 'Star 101.3 — Today\'s Hits', pty: 10, ptyName: 'Pop Music', ms: true },
  104.5: { ps: 'KFOG    ', rt: 'KFOG 104.5 — San Francisco\'s Album Rock', pty: 11, ptyName: 'Rock Music', ms: true },
  107.7: { ps: 'KSAN    ', rt: 'The Bone — Classic Rock', pty: 11, ptyName: 'Rock Music', ms: true },
};

// AM stations (mock)
const MOCK_AM_STATIONS: Record<number, Partial<RdsStationInfo>> = {
  560:  { ps: 'KGO     ', rt: 'KGO 560 — News Talk', pty: 1, ptyName: 'News', ms: false },
  680:  { ps: 'KNBR    ', rt: 'KNBR 680 — Sports Leader', pty: 4, ptyName: 'Sport', ms: false },
  810:  { ps: 'KGO-AM  ', rt: 'ABC News Radio', pty: 1, ptyName: 'News', ms: false },
  960:  { ps: 'KKGN    ', rt: 'Green 960 — Progressive Talk', pty: 2, ptyName: 'Current Affairs', ms: false },
  1010: { ps: 'KIQI    ', rt: 'La Nueva 1010 — Spanish Radio', pty: 0, ptyName: 'None', ms: true },
};

// ── Mock signal — varies slightly over time to feel alive ─────────────────────

let _signalPhase = 0;

function mockSignalForFrequency(hz: number, band: 'fm' | 'am'): SignalInfo {
  _signalPhase += 0.05;
  const stations = band === 'fm' ? MOCK_STATIONS : MOCK_AM_STATIONS;
  const mhz = band === 'fm' ? hz / 1e6 : hz / 1e3;
  const nearest = Object.keys(stations)
    .map(Number)
    .sort((a, b) => Math.abs(a - mhz) - Math.abs(b - mhz))[0];
  const dist = Math.abs((nearest ?? 0) - mhz);
  const onStation = dist < 0.15;

  const baseStrength = onStation ? 0.72 + Math.sin(_signalPhase * 0.3) * 0.08 : 0.08 + Math.random() * 0.12;
  const snr = onStation ? 28 + Math.sin(_signalPhase * 0.2) * 4 : 4 + Math.random() * 6;
  const stereo = onStation && (stations[nearest]?.ms ?? false);

  return {
    strength: Math.max(0, Math.min(1, baseStrength)),
    snr: Math.round(snr * 10) / 10,
    stereo,
    pilotAmplitude: stereo ? 0.2 + Math.sin(_signalPhase * 0.4) * 0.05 : 0,
  };
}

// ── Module ────────────────────────────────────────────────────────────────────

// Captured at load — non-null = native module missing (expected in mock phase)
export const rdsError: string | null = NativeModules.RdsModule
  ? null
  : null; // null intentionally — mock mode is expected and valid during development

/**
 * Get RDS station info for the current frequency.
 * Returns null if no RDS data available (weak signal, AM, etc.)
 *
 * MOCK: Returns canned data matched to frequency.
 * REAL: Will call NativeModules.RdsModule.getStationInfo()
 */
export async function getRdsStationInfo(
  frequencyHz: number,
  band: 'fm' | 'am' = 'fm',
): Promise<RdsStationInfo | null> {
  // REAL: return NativeModules.RdsModule.getStationInfo();

  // AM has no RDS
  if (band === 'am') return null;

  const mhz = frequencyHz / 1e6;
  const stations = MOCK_STATIONS;
  const nearest = Object.keys(stations)
    .map(Number)
    .sort((a, b) => Math.abs(a - mhz) - Math.abs(b - mhz))[0];

  if (!nearest || Math.abs(nearest - mhz) > 0.12) return null;

  const station = stations[nearest];
  return {
    ps: station.ps ?? null,
    rt: station.rt ?? null,
    pty: station.pty ?? 0,
    ptyName: station.ptyName ?? 'None',
    tp: false,
    ta: false,
    ms: station.ms ?? true,
    af: [],
    ct: null,
  };
}

/**
 * Get real-time signal info (strength, SNR, stereo pilot).
 *
 * MOCK: Simulated values that animate slightly.
 * REAL: Will call NativeModules.RdsModule.getSignalInfo() or SdrModule equivalent.
 */
export async function getSignalInfo(
  frequencyHz: number,
  band: 'fm' | 'am' = 'fm',
): Promise<SignalInfo> {
  // REAL: return NativeModules.RdsModule.getSignalInfo();
  return mockSignalForFrequency(frequencyHz, band);
}

/**
 * Get hardware gain info from the RTL-SDR device.
 *
 * MOCK: Returns a typical RTL-SDR Blog V3 gain table.
 * REAL: Will call NativeModules.SdrModule.getGainInfo() after device open.
 */
export async function getHardwareGainInfo(): Promise<HardwareGainInfo> {
  // REAL: return NativeModules.SdrModule.getGainInfo();

  // RTL-SDR Blog V3 gain steps in tenths of dB
  const rtlGains = [
    0, 9, 14, 27, 37, 77, 87, 125, 144, 157, 166, 197,
    207, 229, 254, 280, 297, 328, 338, 364, 372, 386,
    402, 421, 434, 439, 445, 480, 496,
  ];

  return {
    gainTenthsDb: 280,
    availableGains: rtlGains,
    autoGain: false,
  };
}

/**
 * Set hardware gain.
 *
 * MOCK: No-op, returns success.
 * REAL: Will call NativeModules.SdrModule.setGain(tenthsDb, autoGain)
 */
export async function setHardwareGain(
  tenthsDb: number,
  autoGain: boolean = false,
): Promise<boolean> {
  // REAL: return NativeModules.SdrModule.setGain(tenthsDb, autoGain);
  console.log('[RdsModule] setHardwareGain', tenthsDb, 'auto:', autoGain);
  return true;
}

/**
 * Set EQ bands. Values are gain in dB, -12 to +12.
 * Band order: [sub-bass, bass, mid, presence, air]
 * Center freqs: [80Hz, 250Hz, 1kHz, 4kHz, 12kHz]
 *
 * MOCK: No-op, returns success.
 * REAL: Will call NativeModules.SdrModule.setEq(bands)
 */
export async function setEq(bands: number[]): Promise<boolean> {
  // REAL: return NativeModules.SdrModule.setEq(bands);
  console.log('[RdsModule] setEq', bands);
  return true;
}

/**
 * Set mono/stereo mode.
 *
 * MOCK: No-op.
 * REAL: Will call NativeModules.SdrModule.setMonoMode(mono)
 */
export async function setMonoMode(mono: boolean): Promise<boolean> {
  // REAL: return NativeModules.SdrModule.setMonoMode(mono);
  console.log('[RdsModule] setMonoMode', mono);
  return true;
}

/**
 * Start scanning in a direction.
 * Scans until signal threshold is met or band edge wraps.
 *
 * MOCK: Simulates scan by returning next known station frequency.
 * REAL: Will call NativeModules.SdrModule.scan(direction, thresholdDb)
 */
export async function scan(
  currentHz: number,
  direction: 'up' | 'down',
  band: 'fm' | 'am',
): Promise<number> {
  // REAL: return NativeModules.SdrModule.scan(direction, thresholdDb);

  const stationFreqs =
    band === 'fm'
      ? Object.keys(MOCK_STATIONS).map(f => Number(f) * 1e6)
      : Object.keys(MOCK_AM_STATIONS).map(f => Number(f) * 1e3);

  stationFreqs.sort((a, b) => a - b);

  const FM_MIN = 87.5e6, FM_MAX = 108.0e6;
  const AM_MIN = 520e3,  AM_MAX = 1710e3;
  const min = band === 'fm' ? FM_MIN : AM_MIN;
  const max = band === 'fm' ? FM_MAX : AM_MAX;

  if (direction === 'up') {
    const next = stationFreqs.find(f => f > currentHz + 0.05e6);
    return next ?? min; // wrap to bottom
  } else {
    const prev = [...stationFreqs].reverse().find(f => f < currentHz - 0.05e6);
    return prev ?? max; // wrap to top
  }
}

/**
 * Start recording audio to file.
 *
 * MOCK: No-op, returns a simulated filename.
 * REAL: Will call NativeModules.SdrModule.startRecording(filename)
 */
export async function startRecording(filename: string): Promise<boolean> {
  // REAL: return NativeModules.SdrModule.startRecording(filename);
  console.log('[RdsModule] startRecording', filename);
  return true;
}

/**
 * Stop recording audio.
 *
 * MOCK: No-op, returns saved path.
 * REAL: Will call NativeModules.SdrModule.stopRecording()
 */
export async function stopRecording(): Promise<string> {
  // REAL: return NativeModules.SdrModule.stopRecording();
  return '/storage/emulated/0/SDRGo/Recordings/mock_recording.wav';
}

export const EQ_BANDS = [
  { label: 'SUB',  freq: '60Hz',  index: 0 },
  { label: 'BASS', freq: '250Hz', index: 1 },
  { label: 'MUD', freq: '500Hz', index: 2 },
  { label: 'MID',  freq: '1kHz',  index: 3 },
  { label: 'EDGE', freq: '2kHz', index: 4 },
  { label: 'PRES', freq: '10kHz',  index: 5 },
  { label: 'AIR',  freq: '16kHz', index: 6 },
] as const;

export const DEFAULT_EQ: number[] = [0, 0, 0, 0, 0, 0, 0];
export const DEFAULT_GAIN_TENTHS = 280; // 28.0 dB
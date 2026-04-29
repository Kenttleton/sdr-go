import { NativeModules, Platform } from 'react-native';

// Captured at module load time — non-null means the native bridge is missing.
export const driverError: string | null = NativeModules.SdrModule
  ? null
  : 'SdrModule native module not found. Ensure the Android build includes sdr_core and SdrPackage is registered.';

const getNativeModule = () => {
  const { SdrModule } = NativeModules;
  if (!SdrModule) {
    throw new Error(driverError!);
  }
  return SdrModule;
};

const guardAndroid = <T>(fn: () => T): T => {
  if (Platform.OS !== 'android') {
    throw new Error('SdrGo requires Android');
  }
  return fn();
};

export default {
  getCoreVersion: (): Promise<string> =>
    guardAndroid(() => getNativeModule().getCoreVersion()),

  requestUsbPermission: (): Promise<number> =>
    guardAndroid(() => getNativeModule().requestUsbPermission()),

  startFm: (
    fd: number,
    frequencyHz: number,
    audioSampleRate: number = 96000,
    stereo: boolean = true
  ): Promise<boolean> =>
    guardAndroid(() =>
      getNativeModule().startFm(fd, frequencyHz, audioSampleRate, stereo)
    ),

  tuneFrequency: (frequencyHz: number): Promise<boolean> =>
    guardAndroid(() => getNativeModule().tuneFrequency(frequencyHz)),

  stopFm: (): Promise<boolean> =>
    guardAndroid(() => getNativeModule().stopFm()),

  checkStereo: (): Promise<boolean> =>
    guardAndroid(() => getNativeModule().checkStereo()),

  getWaveformBuffer: (): Promise<number[] | null> =>
    guardAndroid(() => getNativeModule().getWaveformBuffer()),
};
import { NativeModules, Platform } from 'react-native';

const { SdrModule } = NativeModules;

if (!SdrModule) {
  throw new Error('SdrModule native module not found.');
}

const guardAndroid = <T>(fn: () => T): T => {
  if (Platform.OS !== 'android') {
    throw new Error('SdrGo requires Android');
  }
  return fn();
};

export default {
  getCoreVersion: (): Promise<string> =>
    guardAndroid(() => SdrModule.getCoreVersion()),

  requestUsbPermission: (): Promise<number> =>
    guardAndroid(() => SdrModule.requestUsbPermission()),

  startFm: (
    fd: number,
    frequencyHz: number,
    audioSampleRate: number = 96000,
    stereo: boolean = true
  ): Promise<boolean> =>
    guardAndroid(() =>
      SdrModule.startFm(fd, frequencyHz, audioSampleRate, stereo)
    ),

  tuneFrequency: (frequencyHz: number): Promise<boolean> =>
    guardAndroid(() => SdrModule.tuneFrequency(frequencyHz)),

  stopFm: (): Promise<boolean> =>
    guardAndroid(() => SdrModule.stopFm()),

  checkStereo: (): Promise<boolean> =>
    guardAndroid(() => SdrModule.checkStereo()),
};
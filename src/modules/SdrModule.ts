import { NativeModules, Platform } from 'react-native';

const { SdrModule } = NativeModules;

if (!SdrModule) {
  throw new Error(
    'SdrModule native module not found. ' +
    'Ensure the Android build includes sdr_core and SdrPackage is registered.'
  );
}

export default {
  getCoreVersion: (): Promise<string> => {
    if (Platform.OS !== 'android') {
      return Promise.reject(new Error('SdrGo requires Android'));
    }
    return SdrModule.getCoreVersion();
  },
};
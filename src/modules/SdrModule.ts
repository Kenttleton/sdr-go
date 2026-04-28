import { NativeModules } from 'react-native';

const { SdrModule } = NativeModules;

export const driverError: string | null = SdrModule
  ? null
  : 'SdrModule native module not found. Ensure the Android build includes sdr_core and SdrPackage is registered.';

export default {
  getCoreVersion: (): Promise<string> => SdrModule.getCoreVersion(),
};
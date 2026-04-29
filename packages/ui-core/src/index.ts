// Theme
export { ThemeProvider, useTheme } from './theme/ThemeContext';
export { colors } from './theme';
export type { Theme, ThemeMode } from './theme';

// Native module wrapper — shared across all apps
export { default as SdrModule, driverError } from './modules/SdrModule';

// Shared hooks
export { useWaveform } from './hooks/useWaveForm';
export { useUsbDevice } from './hooks/useUSBDevice';

// Shared components (added as we build them)
// export { Waveform } from './components/Waveform';
// export { FrequencyDisplay } from './components/FrequencyDisplay';
// export { SignalStrength } from './components/SignalStrength';
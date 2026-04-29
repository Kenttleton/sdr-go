import { useColorScheme } from 'react-native';
import { createContext, useContext, useState } from 'react';

export const colors = {
  dark: {
    background:     '#080a0e',
    surface:        '#0f1318',
    surfaceRaised:  '#161c24',
    border:         '#1e2733',
    primary:        '#00e5ff',   // cyan — instrument glow
    primaryDim:     '#00e5ff22',
    accent:         '#ff6b2b',   // orange — warning/active
    accentDim:      '#ff6b2b22',
    success:        '#00ff88',   // green — stereo/lock
    text:           '#e8edf2',
    textSecondary:  '#5a6a7a',
    textDim:        '#2a3a4a',
    waterfall0:     '#000510',   // no signal
    waterfall1:     '#001a4a',
    waterfall2:     '#003090',
    waterfall3:     '#0060d0',
    waterfall4:     '#00a0e0',
    waterfall5:     '#00e0a0',
    waterfall6:     '#80ff40',
    waterfall7:     '#ffff00',
    waterfall8:     '#ff8000',
    waterfall9:     '#ff0000',   // strong signal
  },
  light: {
    background:     '#f0f4f8',
    surface:        '#ffffff',
    surfaceRaised:  '#e8edf2',
    border:         '#d0d8e0',
    primary:        '#0066cc',
    primaryDim:     '#0066cc22',
    accent:         '#cc4400',
    accentDim:      '#cc440022',
    success:        '#008844',
    text:           '#0a1520',
    textSecondary:  '#4a6070',
    textDim:        '#a0b0c0',
    waterfall0:     '#e8f0f8',
    waterfall1:     '#b0c8e8',
    waterfall2:     '#6090d0',
    waterfall3:     '#2060c0',
    waterfall4:     '#0040a0',
    waterfall5:     '#008080',
    waterfall6:     '#40a040',
    waterfall7:     '#c0c000',
    waterfall8:     '#c06000',
    waterfall9:     '#c00000',
  },
};

export type Theme = typeof colors.dark;
export type ThemeMode = 'dark' | 'light' | 'system';
/**
 * usePresets.ts
 * Persistent FM/AM preset management via AsyncStorage.
 */

import { useState, useEffect, useCallback } from 'react';
import AsyncStorage from '@react-native-async-storage/async-storage';
import { DEFAULT_EQ, DEFAULT_GAIN_TENTHS } from '../modules/RdsModule';

export interface Preset {
  id: string;
  frequencyHz: number;
  band: 'fm' | 'am';
  name: string;           // User-assigned or RDS-derived
  rdsPs: string | null;   // RDS Programme Service name at time of save
  gainTenthsDb: number;
  eq: number[];           // 5 bands
  mono: boolean;
  savedAt: number;        // timestamp ms
}

const STORAGE_KEY = '@sdrgo/presets';

function makeId() {
  return Math.random().toString(36).slice(2) + Date.now().toString(36);
}

export function usePresets() {
  const [presets, setPresets] = useState<Preset[]>([]);
  const [loaded, setLoaded] = useState(false);

  // Load from storage on mount
  useEffect(() => {
    AsyncStorage.getItem(STORAGE_KEY).then(raw => {
      if (raw) {
        try { setPresets(JSON.parse(raw)); } catch (_) {}
      }
      setLoaded(true);
    });
  }, []);

  // Persist whenever presets change (after initial load)
  useEffect(() => {
    if (!loaded) return;
    AsyncStorage.setItem(STORAGE_KEY, JSON.stringify(presets));
  }, [presets, loaded]);

  const savePreset = useCallback((
    frequencyHz: number,
    band: 'fm' | 'am',
    name: string,
    rdsPs: string | null,
    gainTenthsDb: number,
    eq: number[],
    mono: boolean,
  ): Preset => {
    const preset: Preset = {
      id: makeId(),
      frequencyHz,
      band,
      name,
      rdsPs,
      gainTenthsDb,
      eq: [...eq],
      mono,
      savedAt: Date.now(),
    };
    setPresets(prev => {
      // Replace if same frequency+band already saved, otherwise append
      const idx = prev.findIndex(p => p.frequencyHz === frequencyHz && p.band === band);
      if (idx >= 0) {
        const next = [...prev];
        next[idx] = preset;
        return next;
      }
      return [...prev, preset];
    });
    return preset;
  }, []);

  const updatePreset = useCallback((id: string, changes: Partial<Preset>) => {
    setPresets(prev => prev.map(p => p.id === id ? { ...p, ...changes } : p));
  }, []);

  const deletePreset = useCallback((id: string) => {
    setPresets(prev => prev.filter(p => p.id !== id));
  }, []);

  const getPresetsForBand = useCallback((band: 'fm' | 'am') => {
    return presets
      .filter(p => p.band === band)
      .sort((a, b) => a.frequencyHz - b.frequencyHz);
  }, [presets]);

  const findPreset = useCallback((frequencyHz: number, band: 'fm' | 'am') => {
    return presets.find(p => p.frequencyHz === frequencyHz && p.band === band) ?? null;
  }, [presets]);

  return {
    presets,
    loaded,
    savePreset,
    updatePreset,
    deletePreset,
    getPresetsForBand,
    findPreset,
  };
}
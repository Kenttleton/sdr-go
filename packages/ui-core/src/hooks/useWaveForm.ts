import { useState, useEffect, useRef, useCallback } from 'react';
import SdrModule from '../modules/SdrModule';

const WAVEFORM_POINTS = 512;
const EMPTY = Array(WAVEFORM_POINTS).fill(0);

export function useWaveform(active: boolean) {
  const [data, setData] = useState<number[]>(EMPTY);
  const timer = useRef<ReturnType<typeof setInterval> | null>(null);

  const start = useCallback(() => {
    timer.current = setInterval(async () => {
      try {
        const buf = await SdrModule.getWaveformBuffer();
        if (buf && buf.length > 0) setData(buf);
      } catch (_) {}
    }, 16);
  }, []);

  const stop = useCallback(() => {
    if (timer.current) {
      clearInterval(timer.current);
      timer.current = null;
    }
    setData(EMPTY);
  }, []);

  useEffect(() => {
    if (active) {
      start();
    } else {
      stop();
    }
    return stop;
  }, [active]);

  return data;
}
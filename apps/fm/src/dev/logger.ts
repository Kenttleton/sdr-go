import { useState, useEffect } from 'react';

export type LogLevel = 'log' | 'warn' | 'error';

export interface LogEntry {
  id: number;
  level: LogLevel;
  message: string;
  time: string; // HH:MM:SS
}

const MAX_ENTRIES = 300;
const _entries: LogEntry[] = [];
let _id = 0;
const _listeners = new Set<() => void>();

function _push(level: LogLevel, args: unknown[]) {
  const message = args
    .map(a => {
      if (typeof a === 'string') return a;
      try { return JSON.stringify(a); } catch { return String(a); }
    })
    .join(' ');

  const d = new Date();
  const time = [d.getHours(), d.getMinutes(), d.getSeconds()]
    .map(n => String(n).padStart(2, '0'))
    .join(':');

  _entries.push({ id: _id++, level, message, time });
  if (_entries.length > MAX_ENTRIES) _entries.shift();
  _listeners.forEach(fn => fn());
}

export function getLogEntries(): LogEntry[] {
  return _entries.slice();
}

export function clearLogEntries() {
  _entries.length = 0;
  _listeners.forEach(fn => fn());
}

export function subscribeToLogs(fn: () => void): () => void {
  _listeners.add(fn);
  return () => _listeners.delete(fn);
}

export function useDevLogs() {
  const [entries, setEntries] = useState<LogEntry[]>(getLogEntries);
  useEffect(() => subscribeToLogs(() => setEntries(getLogEntries())), []);
  return { entries, clear: clearLogEntries };
}

// ── Auto-init: patch console methods once at module load ──────────────────────
// Imported as a side-effect from RadioScreen so capture starts before the
// settings sheet is ever opened. No-ops in production (__DEV__ = false).
if (__DEV__) {
  const _orig = {
    log:   console.log.bind(console),
    warn:  console.warn.bind(console),
    error: console.error.bind(console),
  };
  console.log   = (...args: unknown[]) => { _orig.log(...args);   _push('log',   args); };
  console.warn  = (...args: unknown[]) => { _orig.warn(...args);  _push('warn',  args); };
  console.error = (...args: unknown[]) => { _orig.error(...args); _push('error', args); };
}

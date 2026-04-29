import { useState, useCallback } from 'react';
import SdrModule from '../modules/SdrModule';

type DeviceState = 'disconnected' | 'requesting' | 'connected' | 'error';

export function useUsbDevice() {
  const [state, setState] = useState<DeviceState>('disconnected');
  const [fd, setFd] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  const connect = useCallback(async () => {
    setState('requesting');
    setError(null);
    try {
      const deviceFd = await SdrModule.requestUsbPermission();
      setFd(deviceFd);
      setState('connected');
      return deviceFd;
    } catch (e: any) {
      setError(e.message);
      setState('error');
      return null;
    }
  }, []);

  const disconnect = useCallback(() => {
    setFd(null);
    setState('disconnected');
    setError(null);
  }, []);

  return {
    state,
    fd,
    error,
    isConnected: state === 'connected',
    connect,
    disconnect,
  };
}
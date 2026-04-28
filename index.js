import React from 'react';
import { registerRootComponent } from 'expo';
import { View, Text, StyleSheet } from 'react-native';
import { driverError } from './src/modules/SdrModule';
import App from './App';

function DriverErrorScreen() {
  return (
    <View style={styles.container}>
      <Text style={styles.title}>SDRGo</Text>
      <Text style={styles.error}>{driverError}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: '#0a0a0a', alignItems: 'center', justifyContent: 'center', padding: 24 },
  title: { fontSize: 42, fontWeight: '700', color: '#00ff88', letterSpacing: 4, marginBottom: 16 },
  error: { fontSize: 14, color: '#ff4444', fontFamily: 'monospace', textAlign: 'center' },
});

registerRootComponent(driverError ? DriverErrorScreen : App);

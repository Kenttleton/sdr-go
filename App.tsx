import React, { useEffect, useState } from "react";
import { StatusBar, StyleSheet, Text, View } from "react-native";
import { SafeAreaProvider, SafeAreaView } from "react-native-safe-area-context";
import SdrModule from "./src/modules/SdrModule";

export default function App() {
  const [coreVersion, setCoreVersion] = useState<string>("Loading...");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    SdrModule.getCoreVersion()
      .then(setCoreVersion)
      .catch((e: Error) => setError(e.message));
  }, []);

  return (
    <SafeAreaProvider>
      <SafeAreaView style={styles.container}>
        <StatusBar barStyle="light-content" backgroundColor="#0a0a0a" />
        <View style={styles.inner}>
          <Text style={styles.title}>SDRGo</Text>
          <Text style={styles.label}>Pipeline Status</Text>
          {error ? (
            <Text style={styles.error}>{error}</Text>
          ) : (
            <Text style={styles.version}>{coreVersion}</Text>
          )}
        </View>
      </SafeAreaView>
    </SafeAreaProvider>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: "#0a0a0a",
  },
  inner: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    gap: 12,
  },
  title: {
    fontSize: 42,
    fontWeight: "700",
    color: "#00ff88",
    letterSpacing: 4,
  },
  label: {
    fontSize: 14,
    color: "#666",
    letterSpacing: 2,
    textTransform: "uppercase",
  },
  version: {
    fontSize: 16,
    color: "#aaa",
    fontFamily: "monospace",
  },
  error: {
    fontSize: 14,
    color: "#ff4444",
    fontFamily: "monospace",
    paddingHorizontal: 24,
    textAlign: "center",
  },
});

import React from "react";
import { View, Text, StyleSheet } from "react-native";
import { useTheme } from "@sdrgo/ui-core";

export default function FmScreen() {
  const { theme } = useTheme();
  return (
    <View style={[styles.container, { backgroundColor: theme.background }]}>
      <Text style={[styles.text, { color: theme.primary }]}>NOAA</Text>
      <Text style={[styles.sub, { color: theme.textSecondary }]}>
        Coming next
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, alignItems: "center", justifyContent: "center" },
  text: { fontSize: 24, fontWeight: "700" },
  sub: { fontSize: 14, marginTop: 8 },
});

/**
 * SignalMeter.tsx
 * Segmented signal strength bar + SNR readout
 * Styled like an aftermarket car stereo signal indicator
 */

import React, { useEffect, useRef } from "react";
import { View, Text, StyleSheet, Animated } from "react-native";
import type { Theme } from "@sdrgo/ui-core";

interface Props {
  strength: number; // 0.0–1.0
  snr: number; // dB
  theme: Theme;
}

const SEGMENT_COUNT = 12;

// Colour zones: green (good) → amber (ok) → red (weak/saturated)
function segmentColor(idx: number, total: number, theme: Theme): string {
  const ratio = idx / total;
  if (ratio < 0.5) return theme.signal; // green
  if (ratio < 0.8) return theme.primary; // amber
  return theme.danger; // red (overload zone)
}

export default function SignalMeter({ strength, snr, theme }: Props) {
  const animStrength = useRef(new Animated.Value(0)).current;

  useEffect(() => {
    Animated.spring(animStrength, {
      toValue: strength,
      tension: 60,
      friction: 8,
      useNativeDriver: false,
    }).start();
  }, [strength]);

  const filledSegments = Math.round(strength * SEGMENT_COUNT);

  return (
    <View style={styles.container}>
      {/* Segments */}
      <View style={styles.segments}>
        {Array.from({ length: SEGMENT_COUNT }).map((_, i) => {
          const filled = i < filledSegments;
          const color = segmentColor(i, SEGMENT_COUNT, theme);
          return (
            <View
              key={i}
              style={[
                styles.segment,
                {
                  backgroundColor: filled ? color : theme.surfaceRaised,
                  shadowColor: filled ? color : "transparent",
                  shadowOpacity: filled ? 0.8 : 0,
                  shadowRadius: filled ? 3 : 0,
                  elevation: filled ? 2 : 0,
                },
              ]}
            />
          );
        })}
      </View>

      {/* SNR readout */}
      <Text style={[styles.snr, { color: theme.textSecondary }]}>
        {snr > 0 ? `${snr.toFixed(1)} dB` : "—"}
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flexDirection: "row",
    alignItems: "center",
    gap: 12,
  },
  segments: {
    flexDirection: "row",
    gap: 3,
    alignItems: "flex-end",
  },
  segment: {
    width: 10,
    borderRadius: 2,
    height: 14,
  },
  snr: {
    fontSize: 10,
    fontFamily: "monospace",
    letterSpacing: 0.5,
    minWidth: 56,
  },
});

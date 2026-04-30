/**
 * Toast.tsx
 * Animated slide-up notification toast
 */

import React, { useEffect, useRef } from "react";
import { View, Text, StyleSheet, Animated } from "react-native";
import type { Theme } from "@sdrgo/ui-core";

interface Props {
  message: string;
  sub?: string;
  theme: Theme;
  insets: { bottom: number };
}

export default function Toast({ message, sub, theme, insets }: Props) {
  const anim = useRef(new Animated.Value(0)).current;

  useEffect(() => {
    Animated.sequence([
      Animated.spring(anim, {
        toValue: 1,
        tension: 60,
        friction: 10,
        useNativeDriver: true,
      }),
      Animated.delay(2800),
      Animated.timing(anim, {
        toValue: 0,
        duration: 300,
        useNativeDriver: true,
      }),
    ]).start();
  }, []);

  const translateY = anim.interpolate({
    inputRange: [0, 1],
    outputRange: [80, 0],
  });

  return (
    <Animated.View
      style={[
        styles.container,
        {
          backgroundColor: theme.surfaceRaised,
          borderColor: theme.border,
          bottom: insets.bottom + 100, // above dial
          transform: [{ translateY }],
          opacity: anim,
        },
      ]}
      pointerEvents="none"
    >
      <View style={[styles.accent, { backgroundColor: theme.primary }]} />
      <View style={styles.textCol}>
        <Text style={[styles.message, { color: theme.text }]} numberOfLines={1}>
          {message}
        </Text>
        {sub && (
          <Text
            style={[styles.sub, { color: theme.textSecondary }]}
            numberOfLines={1}
          >
            {sub}
          </Text>
        )}
      </View>
    </Animated.View>
  );
}

const styles = StyleSheet.create({
  container: {
    position: "absolute",
    left: 20,
    right: 20,
    borderRadius: 12,
    borderWidth: StyleSheet.hairlineWidth,
    flexDirection: "row",
    alignItems: "center",
    overflow: "hidden",
    paddingRight: 16,
    paddingVertical: 12,
    shadowColor: "#000",
    shadowOffset: { width: 0, height: 4 },
    shadowOpacity: 0.3,
    shadowRadius: 12,
    elevation: 8,
  },
  accent: {
    width: 4,
    alignSelf: "stretch",
    marginRight: 12,
  },
  textCol: {
    flex: 1,
    gap: 2,
  },
  message: {
    fontSize: 13,
    fontWeight: "700",
  },
  sub: {
    fontSize: 11,
  },
});

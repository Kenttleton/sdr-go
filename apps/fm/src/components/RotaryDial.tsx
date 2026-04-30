/**
 * RotaryDial.tsx
 *
 * Rotary tuning dial with:
 * - Horizontal swipe → rotary animation (premium feel)
 * - Velocity-sensitive tuning: slow = fine, fast = coarse
 * - Momentum / deceleration after release
 * - Center play/pause button
 * - EQ button (right side)
 * - Tick marks that scroll with rotation
 */

import React, { useRef, useCallback, useEffect } from 'react';
import {
  View, Text, StyleSheet, TouchableOpacity,
  PanResponder, Animated, Dimensions, Platform,
} from 'react-native';
import { Canvas, Path, Circle, Group } from '@shopify/react-native-skia';
import type { Theme } from '@sdrgo/ui-core';

const { width: SCREEN_W } = Dimensions.get('window');

const DIAL_H = 220;
const DIAL_RADIUS = 320; // virtual radius for the curved feel
const TICK_COUNT = 80;
const TICK_SPACING = SCREEN_W / 10; // pixels between major ticks at rest

// Velocity sensitivity curve
// pixelsPerSecond → frequencyDeltaHz
function velocityToFreqDelta(velocityPxPerMs: number, band: 'fm' | 'am'): number {
  const absPx = Math.abs(velocityPxPerMs);
  const sign = velocityPxPerMs > 0 ? 1 : -1;

  // Quadratic curve: slow drag = fine, fast = coarse
  // FM: 0.1 MHz at slow, up to 2.0 MHz at fast swipe
  // AM: 10 kHz at slow, up to 200 kHz at fast swipe
  const sensitivity = band === 'fm' ? 0.6e6 : 60e3;
  const delta = sign * Math.pow(absPx, 1.4) * sensitivity;

  // Snap to step
  const step = band === 'fm' ? 0.1e6 : 10e3;
  return Math.round(delta / step) * step;
}

interface Props {
  frequencyHz: number;
  band: 'fm' | 'am';
  onFrequencyChange: (hz: number) => void;
  theme: Theme;
  isPlaying: boolean;
  onPlayToggle: () => void;
  onEqPress: () => void;
}

export default function RotaryDial({
  frequencyHz, band, onFrequencyChange,
  theme, isPlaying, onPlayToggle, onEqPress,
}: Props) {
  // Rotation offset in degrees — drives tick mark animation
  const rotation = useRef(new Animated.Value(0)).current;
  const rotationValue = useRef(0);
  const lastDx = useRef(0);
  const velocityBuffer = useRef<number[]>([]);
  const momentumAnim = useRef<Animated.CompositeAnimation | null>(null);
  const freqRef = useRef(frequencyHz);
  const bandRef = useRef(band);

  useEffect(() => { freqRef.current = frequencyHz; }, [frequencyHz]);
  useEffect(() => { bandRef.current = band; }, [band]);

  // Track rotation value for tick calculations
  useEffect(() => {
    const id = rotation.addListener(({ value }) => {
      rotationValue.current = value;
    });
    return () => rotation.removeListener(id);
  }, []);

  const applyFreqDelta = useCallback((pxDelta: number) => {
    // px → velocity → freq delta
    // Treat dx as pixels dragged (not velocity for now; velocity applied on release)
    const step = bandRef.current === 'fm' ? 0.1e6 : 10e3;
    // Fine mode: ~30px per step at slow drag, fewer at fast drag
    const stepsPerPx = 0.033;
    const rawSteps = pxDelta * stepsPerPx;
    const steps = Math.round(rawSteps);
    if (steps === 0) return;
    const delta = -steps * step; // drag right = frequency up
    const FM_MIN = 87.5e6, FM_MAX = 108.0e6;
    const AM_MIN = 520e3,  AM_MAX = 1710e3;
    const min = bandRef.current === 'fm' ? FM_MIN : AM_MIN;
    const max = bandRef.current === 'fm' ? FM_MAX : AM_MAX;
    const next = Math.max(min, Math.min(max, freqRef.current + delta));
    onFrequencyChange(next);
  }, [onFrequencyChange]);

  const panResponder = useRef(
    PanResponder.create({
      onStartShouldSetPanResponder: () => true,
      onMoveShouldSetPanResponder: (_, gs) => Math.abs(gs.dx) > 3,

      onPanResponderGrant: () => {
        momentumAnim.current?.stop();
        lastDx.current = 0;
        velocityBuffer.current = [];
      },

      onPanResponderMove: (_, gs) => {
        const dx = gs.dx - lastDx.current;
        lastDx.current = gs.dx;

        // Rotate dial visual — 1px drag = 0.5deg rotation
        const newRot = rotationValue.current - dx * 0.5;
        rotation.setValue(newRot);
        rotationValue.current = newRot;

        // Collect recent velocities for momentum
        velocityBuffer.current.push(gs.vx);
        if (velocityBuffer.current.length > 5) velocityBuffer.current.shift();

        // Apply frequency change for each pixel crossed
        applyFreqDelta(dx);
      },

      onPanResponderRelease: (_, gs) => {
        // Average recent velocity for smoother momentum
        const avgVx = velocityBuffer.current.length
          ? velocityBuffer.current.reduce((a, b) => a + b, 0) / velocityBuffer.current.length
          : gs.vx;

        if (Math.abs(avgVx) > 0.1) {
          // Momentum: spin down over ~600ms
          const targetRot = rotationValue.current - avgVx * 200;
          momentumAnim.current = Animated.timing(rotation, {
            toValue: targetRot,
            duration: 600,
            useNativeDriver: true,
          });
          momentumAnim.current.start();

          // Apply coarse frequency change based on release velocity
          const freqDelta = velocityToFreqDelta(avgVx, bandRef.current);
          const FM_MIN = 87.5e6, FM_MAX = 108.0e6;
          const AM_MIN = 520e3,  AM_MAX = 1710e3;
          const min = bandRef.current === 'fm' ? FM_MIN : AM_MIN;
          const max = bandRef.current === 'fm' ? FM_MAX : AM_MAX;
          const next = Math.max(min, Math.min(max, freqRef.current + freqDelta));
          onFrequencyChange(next);
        }
      },

      onPanResponderTerminate: () => {
        velocityBuffer.current = [];
      },
    })
  ).current;

  // ── Tick marks (drawn via Canvas) ─────────────────────────────────────────
  const ticks = useCallback(() => {
    const paths: string[] = [];
    const W = SCREEN_W;

    for (let i = -TICK_COUNT; i <= TICK_COUNT; i++) {
      const isMajor = i % 5 === 0;
      const isMid = i % 5 === 0 && i % 10 !== 0;
      const x = W / 2 + i * (TICK_SPACING / 5);
      if (x < -10 || x > W + 10) continue;

      const tickH = isMajor ? 28 : 12;
      const y1 = 8;
      const y2 = y1 + tickH;
      paths.push(`M ${x} ${y1} L ${x} ${y2}`);
    }
    return paths;
  }, []);

  const tickPaths = ticks();

  return (
    <View style={styles.container} {...panResponder.panHandlers}>

      {/* Tick marks — static reference layer */}
      <View style={styles.tickArea} pointerEvents="none">
        <Canvas style={{ width: SCREEN_W, height: 60 }}>
          {tickPaths.map((path, i) => {
            const isMajor = i % 5 === 0;
            return (
              <Path
                key={i}
                path={path}
                color={isMajor ? theme.textSecondary : theme.textDim}
                style="stroke"
                strokeWidth={isMajor ? 2 : 1}
              />
            );
          })}
        </Canvas>

        {/* Center needle */}
        <View style={styles.needle} pointerEvents="none">
          <View style={[styles.needleBar, { backgroundColor: theme.primary }]} />
          <View style={[styles.needleArrow, { borderTopColor: theme.primary }]} />
        </View>

        {/* Frequency marker labels */}
        <Animated.View
          style={[styles.freqLabels, {
            transform: [{ translateX: rotation.interpolate({
              inputRange: [-1000, 1000],
              outputRange: [100, -100],
            }) }],
          }]}
          pointerEvents="none"
        >
          {/* These are decorative — real freq is shown in main display */}
          {[-2, -1, 0, 1, 2].map(offset => {
            const step = band === 'fm' ? 1e6 : 100e3;
            const hz = frequencyHz + offset * step;
            const label = band === 'fm'
              ? (hz / 1e6).toFixed(0)
              : (hz / 1e3).toFixed(0);
            return (
              <Text
                key={offset}
                style={[
                  styles.freqLabel,
                  { color: offset === 0 ? theme.primary : theme.textDim },
                ]}
              >
                {label}
              </Text>
            );
          })}
        </Animated.View>
      </View>

      {/* Bottom row: EQ — Play — spacer */}
      <View style={styles.controls}>

        {/* EQ button */}
        <TouchableOpacity style={styles.sideBtn} onPress={onEqPress}>
          <Text style={[styles.sideBtnLabel, { color: theme.textSecondary }]}>EQ</Text>
          <View style={[styles.sideBtnDots, { backgroundColor: theme.surfaceRaised }]}>
            {[0, 1, 2, 3, 4].map(i => (
              <View
                key={i}
                style={[
                  styles.eqDot,
                  {
                    backgroundColor: theme.primary,
                    height: 6 + i * 4,
                    opacity: 0.4 + i * 0.12,
                  },
                ]}
              />
            ))}
          </View>
        </TouchableOpacity>

        {/* Play / Stop button */}
        <TouchableOpacity
          style={[
            styles.playBtn,
            {
              backgroundColor: isPlaying ? theme.danger : theme.primary,
              shadowColor: isPlaying ? theme.danger : theme.primary,
            },
          ]}
          onPress={onPlayToggle}
          activeOpacity={0.85}
        >
          <Text style={[styles.playBtnIcon, { color: theme.textInverse }]}>
            {isPlaying ? '■' : '▶'}
          </Text>
        </TouchableOpacity>

        {/* Spacer / drag hint */}
        <View style={styles.sideBtn}>
          <Text style={[styles.dragHint, { color: theme.textDim }]}>⟵ drag ⟶</Text>
        </View>

      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    overflow: 'hidden',
  },
  tickArea: {
    height: 80,
    position: 'relative',
    overflow: 'hidden',
  },
  needle: {
    position: 'absolute',
    top: 0,
    left: SCREEN_W / 2 - 1,
    alignItems: 'center',
    pointerEvents: 'none',
  },
  needleBar: {
    width: 2,
    height: 36,
  },
  needleArrow: {
    width: 0,
    height: 0,
    borderLeftWidth: 5,
    borderRightWidth: 5,
    borderTopWidth: 7,
    borderLeftColor: 'transparent',
    borderRightColor: 'transparent',
  },
  freqLabels: {
    position: 'absolute',
    bottom: 4,
    left: 0,
    right: 0,
    flexDirection: 'row',
    justifyContent: 'center',
    gap: 40,
  },
  freqLabel: {
    fontSize: 10,
    fontFamily: Platform.select({ ios: 'Courier New', android: 'monospace' }),
    fontWeight: '600',
    letterSpacing: 0.5,
  },
  controls: {
    flex: 1,
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: 28,
    paddingBottom: 16,
  },
  sideBtn: {
    width: 80,
    alignItems: 'center',
    gap: 6,
  },
  sideBtnLabel: {
    fontSize: 10,
    fontWeight: '800',
    letterSpacing: 2,
  },
  sideBtnDots: {
    flexDirection: 'row',
    alignItems: 'flex-end',
    gap: 3,
    paddingHorizontal: 8,
    paddingVertical: 6,
    borderRadius: 8,
    height: 36,
  },
  eqDot: {
    width: 4,
    borderRadius: 2,
  },
  playBtn: {
    width: 72,
    height: 72,
    borderRadius: 36,
    alignItems: 'center',
    justifyContent: 'center',
    shadowOffset: { width: 0, height: 0 },
    shadowOpacity: 0.5,
    shadowRadius: 16,
    elevation: 8,
  },
  playBtnIcon: {
    fontSize: 24,
    fontWeight: '900',
    marginLeft: 2,
  },
  dragHint: {
    fontSize: 10,
    letterSpacing: 1,
  },
});
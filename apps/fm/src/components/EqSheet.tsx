/**
 * EqSheet.tsx
 *
 * Half-screen modal sheet for active audio settings:
 *   - 5-band parametric EQ (-12 to +12 dB)
 *   - Hardware gain control (RTL-SDR gain table)
 *   - Mono / Stereo toggle
 *
 * Audio continues playing while this is open.
 * Settings are saved into presets per-station.
 */

import React, { useRef, useEffect, useState, useCallback } from "react";
import {
  View,
  Text,
  StyleSheet,
  Modal,
  TouchableOpacity,
  ScrollView,
  PanResponder,
  Animated,
  Dimensions,
  Platform,
} from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { EQ_BANDS, getHardwareGainInfo } from "@sdrgo/ui-core";
import type { Theme } from "@sdrgo/ui-core";

const { height: SCREEN_H, width: SCREEN_W } = Dimensions.get("window");
const SHEET_H = SCREEN_H * 0.52;
const SLIDER_H = 140; // px — height of vertical EQ sliders
const EQ_MIN = -12;
const EQ_MAX = 12;

// ── Vertical EQ slider ────────────────────────────────────────────────────────
interface SliderProps {
  value: number; // -12..12 dB
  label: string;
  freq: string;
  onChange: (v: number) => void;
  theme: Theme;
}

function EqSlider({ value, label, freq, onChange, theme }: SliderProps) {
  const anim = useRef(new Animated.Value(value)).current;
  const animValue = useRef(value);
  const startY = useRef(0);
  const startValue = useRef(value);

  useEffect(() => {
    Animated.spring(anim, {
      toValue: value,
      tension: 80,
      friction: 10,
      useNativeDriver: false,
    }).start();
    animValue.current = value;
  }, [value]);

  // Value → pixel position (top of thumb from top of track)
  const valueToY = (v: number) => {
    const ratio = 1 - (v - EQ_MIN) / (EQ_MAX - EQ_MIN);
    return ratio * SLIDER_H;
  };

  // Pixel → value
  const yToValue = (y: number) => {
    const ratio = 1 - y / SLIDER_H;
    const raw = ratio * (EQ_MAX - EQ_MIN) + EQ_MIN;
    // Snap to 0.5 dB steps
    return Math.round(raw * 2) / 2;
  };

  const pan = useRef(
    PanResponder.create({
      onStartShouldSetPanResponder: () => true,
      onPanResponderGrant: (_, gs) => {
        startY.current = gs.y0;
        startValue.current = animValue.current;
      },
      onPanResponderMove: (_, gs) => {
        const dy = gs.dy;
        const newY = Math.max(
          0,
          Math.min(SLIDER_H, valueToY(startValue.current) + dy),
        );
        const newVal = Math.max(EQ_MIN, Math.min(EQ_MAX, yToValue(newY)));
        anim.setValue(newVal);
        animValue.current = newVal;
        onChange(newVal);
      },
    }),
  ).current;

  const thumbTop = anim.interpolate({
    inputRange: [EQ_MIN, EQ_MAX],
    outputRange: [SLIDER_H, 0],
  });

  const positiveH = anim.interpolate({
    inputRange: [EQ_MIN, 0, EQ_MAX],
    outputRange: [0, 0, SLIDER_H / 2],
  });

  const negativeH = anim.interpolate({
    inputRange: [EQ_MIN, 0, EQ_MAX],
    outputRange: [SLIDER_H / 2, 0, 0],
  });

  const isBoost = value >= 0;

  return (
    <View style={eqStyles.sliderWrap} {...pan.panHandlers}>
      {/* dB label */}
      <Text
        style={[
          eqStyles.dbLabel,
          { color: isBoost ? theme.signal : theme.danger },
        ]}
      >
        {value > 0 ? "+" : ""}
        {value.toFixed(1)}
      </Text>

      {/* Track */}
      <View style={[eqStyles.track, { backgroundColor: theme.surfaceRaised }]}>
        {/* Zero line */}
        <View style={[eqStyles.zeroLine, { backgroundColor: theme.border }]} />

        {/* Fill above zero (boost) */}
        <Animated.View
          style={[
            eqStyles.fillBoost,
            {
              height: positiveH,
              backgroundColor: theme.signal + "aa",
              bottom: SLIDER_H / 2,
            },
          ]}
        />

        {/* Fill below zero (cut) */}
        <Animated.View
          style={[
            eqStyles.fillCut,
            {
              height: negativeH,
              backgroundColor: theme.danger + "aa",
              top: SLIDER_H / 2,
            },
          ]}
        />

        {/* Thumb */}
        <Animated.View
          style={[
            eqStyles.thumb,
            {
              top: thumbTop,
              backgroundColor: theme.primary,
              shadowColor: theme.primary,
            },
          ]}
        />
      </View>

      {/* Labels */}
      <Text style={[eqStyles.bandLabel, { color: theme.primary }]}>
        {label}
      </Text>
      <Text style={[eqStyles.freqLabel, { color: theme.textDim }]}>{freq}</Text>
    </View>
  );
}

const eqStyles = StyleSheet.create({
  sliderWrap: {
    alignItems: "center",
    gap: 4,
    paddingHorizontal: 4,
  },
  dbLabel: {
    fontSize: 9,
    fontFamily: "monospace",
    fontWeight: "600",
    letterSpacing: 0.3,
    height: 14,
  },
  track: {
    width: 12,
    height: SLIDER_H,
    borderRadius: 6,
    overflow: "hidden",
    position: "relative",
  },
  zeroLine: {
    position: "absolute",
    left: 0,
    right: 0,
    top: SLIDER_H / 2,
    height: 1,
  },
  fillBoost: {
    position: "absolute",
    left: 0,
    right: 0,
  },
  fillCut: {
    position: "absolute",
    left: 0,
    right: 0,
  },
  thumb: {
    position: "absolute",
    left: -4,
    width: 20,
    height: 20,
    borderRadius: 10,
    marginTop: -10,
    shadowOffset: { width: 0, height: 0 },
    shadowOpacity: 0.8,
    shadowRadius: 6,
    elevation: 4,
  },
  bandLabel: {
    fontSize: 9,
    fontWeight: "800",
    letterSpacing: 1.5,
    marginTop: 2,
  },
  freqLabel: {
    fontSize: 8,
    fontFamily: "monospace",
  },
});

// ── Gain knob (horizontal scrubber) ──────────────────────────────────────────
interface GainKnobProps {
  gainTenthsDb: number;
  availableGains: number[];
  onGainChange: (v: number) => void;
  theme: Theme;
}

function GainKnob({
  gainTenthsDb,
  availableGains,
  onGainChange,
  theme,
}: GainKnobProps) {
  const startX = useRef(0);
  const startGain = useRef(gainTenthsDb);
  const gainRef = useRef(gainTenthsDb);

  useEffect(() => {
    gainRef.current = gainTenthsDb;
  }, [gainTenthsDb]);

  const pan = useRef(
    PanResponder.create({
      onStartShouldSetPanResponder: () => true,
      onPanResponderGrant: (_, gs) => {
        startX.current = gs.x0;
        startGain.current = gainRef.current;
      },
      onPanResponderMove: (_, gs) => {
        const dx = gs.dx;
        const maxGain = availableGains[availableGains.length - 1];
        const minGain = availableGains[0];
        const range = maxGain - minGain;
        const delta = (dx / (SCREEN_W * 0.7)) * range;
        const raw = startGain.current + delta;
        // Snap to nearest available gain step
        const snapped = availableGains.reduce((prev, curr) =>
          Math.abs(curr - raw) < Math.abs(prev - raw) ? curr : prev,
        );
        gainRef.current = snapped;
        onGainChange(snapped);
      },
    }),
  ).current;

  const maxGain = availableGains.length
    ? availableGains[availableGains.length - 1]
    : 496;
  const minGain = availableGains.length ? availableGains[0] : 0;
  const fillRatio = availableGains.length
    ? (gainTenthsDb - minGain) / (maxGain - minGain)
    : 0;

  return (
    <View style={gainStyles.container} {...pan.panHandlers}>
      <View style={gainStyles.header}>
        <Text style={[gainStyles.label, { color: theme.textSecondary }]}>
          GAIN
        </Text>
        <Text style={[gainStyles.value, { color: theme.primary }]}>
          {(gainTenthsDb / 10).toFixed(1)} dB
        </Text>
      </View>

      {/* Gain bar */}
      <View
        style={[gainStyles.track, { backgroundColor: theme.surfaceRaised }]}
      >
        <View
          style={[
            gainStyles.fill,
            {
              width: `${fillRatio * 100}%`,
              backgroundColor: theme.primary + "cc",
            },
          ]}
        />
        {/* Tick marks for gain steps */}
        {availableGains
          .filter((_, i) => i % 4 === 0)
          .map((g) => {
            const ratio = (g - minGain) / (maxGain - minGain);
            return (
              <View
                key={g}
                style={[
                  gainStyles.tick,
                  {
                    left: `${ratio * 100}%`,
                    backgroundColor: theme.border,
                  },
                ]}
              />
            );
          })}
        {/* Thumb */}
        <View
          style={[
            gainStyles.thumb,
            {
              left: `${fillRatio * 100}%`,
              backgroundColor: theme.primary,
              shadowColor: theme.primary,
              transform: [{ translateX: -10 }],
            },
          ]}
        />
      </View>

      <View style={gainStyles.rangeRow}>
        <Text style={[gainStyles.rangeLabel, { color: theme.textDim }]}>
          {(minGain / 10).toFixed(0)} dB
        </Text>
        <Text style={[gainStyles.hint, { color: theme.textDim }]}>
          ⟵ drag ⟶
        </Text>
        <Text style={[gainStyles.rangeLabel, { color: theme.textDim }]}>
          {(maxGain / 10).toFixed(1)} dB
        </Text>
      </View>
    </View>
  );
}

const gainStyles = StyleSheet.create({
  container: {
    gap: 8,
    paddingHorizontal: 4,
  },
  header: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "baseline",
  },
  label: {
    fontSize: 10,
    fontWeight: "800",
    letterSpacing: 2,
  },
  value: {
    fontSize: 14,
    fontWeight: "700",
    fontFamily: "monospace",
  },
  track: {
    height: 10,
    borderRadius: 5,
    overflow: "visible",
    position: "relative",
  },
  fill: {
    height: "100%",
    borderRadius: 5,
  },
  tick: {
    position: "absolute",
    width: 1,
    height: 14,
    top: -2,
  },
  thumb: {
    position: "absolute",
    top: -5,
    width: 20,
    height: 20,
    borderRadius: 10,
    shadowOffset: { width: 0, height: 0 },
    shadowOpacity: 0.7,
    shadowRadius: 6,
    elevation: 4,
  },
  rangeRow: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
  },
  rangeLabel: {
    fontSize: 9,
    fontFamily: "monospace",
  },
  hint: {
    fontSize: 9,
    letterSpacing: 1,
  },
});

// ── Main EqSheet ──────────────────────────────────────────────────────────────

interface EqSheetProps {
  visible: boolean;
  onClose: () => void;
  eq: number[];
  onEqChange: (bands: number[]) => void;
  gainTenthsDb: number;
  onGainChange: (tenthsDb: number) => void;
  isMono: boolean;
  onMonoChange: (mono: boolean) => void;
  isStereoAvailable: boolean;
  theme: Theme;
}

export default function EqSheet({
  visible,
  onClose,
  eq,
  onEqChange,
  gainTenthsDb,
  onGainChange,
  isMono,
  onMonoChange,
  isStereoAvailable,
  theme,
}: EqSheetProps) {
  const insets = useSafeAreaInsets();
  const slideAnim = useRef(new Animated.Value(SHEET_H)).current;
  const [availableGains, setAvailableGains] = useState<number[]>([0, 280, 496]);

  useEffect(() => {
    getHardwareGainInfo().then((info) =>
      setAvailableGains(info.availableGains),
    );
  }, []);

  useEffect(() => {
    Animated.spring(slideAnim, {
      toValue: visible ? 0 : SHEET_H,
      tension: 70,
      friction: 14,
      useNativeDriver: true,
    }).start();
  }, [visible]);

  const handleEqBandChange = useCallback(
    (idx: number, value: number) => {
      const next = [...eq];
      next[idx] = value;
      onEqChange(next);
    },
    [eq, onEqChange],
  );

  const handleReset = useCallback(() => {
    onEqChange([0, 0, 0, 0, 0, 0, 0]);
  }, [onEqChange]);

  if (!visible && slideAnim === undefined) return null;

  return (
    <Modal
      visible={visible}
      transparent
      animationType="none"
      onRequestClose={onClose}
    >
      <TouchableOpacity
        style={sheet.backdrop}
        activeOpacity={1}
        onPress={onClose}
      />

      <Animated.View
        style={[
          sheet.container,
          {
            backgroundColor: theme.overlay,
            borderTopColor: theme.border,
            paddingBottom: insets.bottom + 16,
            height: SHEET_H,
            transform: [{ translateY: slideAnim }],
          },
        ]}
      >
        {/* Handle */}
        <View style={[sheet.handle, { backgroundColor: theme.border }]} />

        {/* Header */}
        <View style={sheet.header}>
          <Text style={[sheet.title, { color: theme.text }]}>
            AUDIO SETTINGS
          </Text>
          <View style={sheet.headerRight}>
            <TouchableOpacity onPress={handleReset}>
              <Text style={[sheet.resetBtn, { color: theme.textSecondary }]}>
                RESET EQ
              </Text>
            </TouchableOpacity>
            <TouchableOpacity onPress={onClose} style={sheet.closeBtn}>
              <Text
                style={[sheet.closeBtnText, { color: theme.textSecondary }]}
              >
                ✕
              </Text>
            </TouchableOpacity>
          </View>
        </View>

        <ScrollView
          showsVerticalScrollIndicator={false}
          contentContainerStyle={sheet.content}
        >
          {/* ── Stereo / Mono toggle ── */}
          <View style={sheet.section}>
            <Text style={[sheet.sectionLabel, { color: theme.textSecondary }]}>
              OUTPUT MODE
            </Text>
            <View style={sheet.monoRow}>
              {[
                { label: "STEREO", value: false },
                { label: "MONO", value: true },
              ].map((opt) => (
                <TouchableOpacity
                  key={opt.label}
                  style={[
                    sheet.modeBtn,
                    {
                      backgroundColor:
                        isMono === opt.value
                          ? theme.primaryGlow
                          : theme.surfaceRaised,
                      borderColor:
                        isMono === opt.value ? theme.primary : theme.border,
                    },
                  ]}
                  onPress={() => onMonoChange(opt.value)}
                >
                  <Text
                    style={[
                      sheet.modeBtnText,
                      {
                        color:
                          isMono === opt.value
                            ? theme.primary
                            : theme.textSecondary,
                      },
                    ]}
                  >
                    {opt.label}
                  </Text>
                  {opt.label === "STEREO" && isStereoAvailable && (
                    <View
                      style={[
                        sheet.stereoIndicator,
                        { backgroundColor: theme.signal },
                      ]}
                    />
                  )}
                </TouchableOpacity>
              ))}
              {isMono === false && isStereoAvailable && (
                <Text style={[sheet.stereoNote, { color: theme.signal }]}>
                  ● stereo pilot detected
                </Text>
              )}
            </View>
          </View>

          {/* ── EQ sliders ── */}
          <View style={sheet.section}>
            <Text style={[sheet.sectionLabel, { color: theme.textSecondary }]}>
              EQUALIZER
            </Text>
            <View style={sheet.eqRow}>
              {/* Scale labels */}
              <View style={sheet.eqScale}>
                {["+12", "+6", "0", "−6", "−12"].map((l) => (
                  <Text
                    key={l}
                    style={[sheet.eqScaleLabel, { color: theme.textDim }]}
                  >
                    {l}
                  </Text>
                ))}
              </View>
              {/* Sliders */}
              {EQ_BANDS.map((band) => (
                <EqSlider
                  key={band.index}
                  value={eq[band.index]}
                  label={band.label}
                  freq={band.freq}
                  onChange={(v) => handleEqBandChange(band.index, v)}
                  theme={theme}
                />
              ))}
            </View>
          </View>

          {/* ── Gain ── */}
          <View style={sheet.section}>
            <Text style={[sheet.sectionLabel, { color: theme.textSecondary }]}>
              HARDWARE GAIN (RTL-SDR)
            </Text>
            <GainKnob
              gainTenthsDb={gainTenthsDb}
              availableGains={availableGains}
              onGainChange={onGainChange}
              theme={theme}
            />
          </View>
        </ScrollView>
      </Animated.View>
    </Modal>
  );
}

const sheet = StyleSheet.create({
  backdrop: {
    flex: 1,
    backgroundColor: "#00000055",
  },
  container: {
    position: "absolute",
    bottom: 0,
    left: 0,
    right: 0,
    borderTopWidth: StyleSheet.hairlineWidth,
    borderTopLeftRadius: 20,
    borderTopRightRadius: 20,
    paddingTop: 12,
    overflow: "hidden",
  },
  handle: {
    width: 40,
    height: 4,
    borderRadius: 2,
    alignSelf: "center",
    marginBottom: 12,
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: 20,
    marginBottom: 16,
  },
  title: {
    fontSize: 11,
    fontWeight: "800",
    letterSpacing: 2.5,
  },
  headerRight: {
    flexDirection: "row",
    alignItems: "center",
    gap: 16,
  },
  resetBtn: {
    fontSize: 10,
    fontWeight: "700",
    letterSpacing: 1.5,
  },
  closeBtn: {
    width: 28,
    height: 28,
    alignItems: "center",
    justifyContent: "center",
  },
  closeBtnText: {
    fontSize: 16,
  },
  content: {
    paddingHorizontal: 20,
    gap: 24,
    paddingBottom: 8,
  },
  section: {
    gap: 12,
  },
  sectionLabel: {
    fontSize: 9,
    fontWeight: "800",
    letterSpacing: 2.5,
  },
  monoRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 10,
    flexWrap: "wrap",
  },
  modeBtn: {
    flexDirection: "row",
    alignItems: "center",
    paddingHorizontal: 20,
    paddingVertical: 10,
    borderRadius: 10,
    borderWidth: 1,
    gap: 6,
  },
  modeBtnText: {
    fontSize: 12,
    fontWeight: "800",
    letterSpacing: 2,
  },
  stereoIndicator: {
    width: 6,
    height: 6,
    borderRadius: 3,
  },
  stereoNote: {
    fontSize: 9,
    fontWeight: "600",
    letterSpacing: 0.5,
  },
  eqRow: {
    flexDirection: "row",
    alignItems: "flex-start",
    gap: 10,
    justifyContent: "center",
  },
  eqScale: {
    height: SLIDER_H + 36,
    justifyContent: "space-between",
    alignItems: "flex-end",
    paddingTop: 14,
    paddingBottom: 52,
  },
  eqScaleLabel: {
    fontSize: 8,
    fontFamily: "monospace",
    width: 24,
    textAlign: "right",
  },
});

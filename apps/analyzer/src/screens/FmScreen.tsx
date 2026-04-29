import React, { useEffect, useRef, useState, useCallback } from "react";
import {
  View,
  Text,
  StyleSheet,
  TouchableOpacity,
  ScrollView,
  Dimensions,
  TextInput,
  PanResponder,
  Animated,
} from "react-native";
import { Canvas, Path, Skia } from "@shopify/react-native-skia";
import { useTheme, SdrModule } from "@sdrgo/ui-core";

const { width } = Dimensions.get("window");
const WAVEFORM_WIDTH = width - 32;
const WAVEFORM_HEIGHT = 120;
const WAVEFORM_POINTS = 512;

type WaveformMode = "oscilloscope" | "envelope";
type TuningMode = "dial" | "keypad";

// FM frequency range
const FM_MIN = 87.5;
const FM_MAX = 108.0;
const FM_STEP = 0.1;

interface Preset {
  frequency: number;
  label: string;
}

const DEFAULT_PRESETS: Preset[] = [
  { frequency: 88.5, label: "NPR" },
  { frequency: 91.3, label: "Jazz" },
  { frequency: 95.5, label: "Classic Rock" },
  { frequency: 98.1, label: "Pop" },
  { frequency: 101.1, label: "Country" },
];

export default function FmScreen() {
  const { theme } = useTheme();
  const s = styles(theme);

  // State
  const [frequency, setFrequency] = useState(100.1);
  const [isPlaying, setIsPlaying] = useState(false);
  const [isStereo, setIsStereo] = useState(false);
  const [waveformMode, setWaveformMode] =
    useState<WaveformMode>("oscilloscope");
  const [tuningMode, setTuningMode] = useState<TuningMode>("dial");
  const [keypadInput, setKeypadInput] = useState("");
  const [presets, setPresets] = useState<Preset[]>(DEFAULT_PRESETS);
  const [rdsStation, setRdsStation] = useState("");
  const [rdsText, setRdsText] = useState("");
  const [waveformData, setWaveformData] = useState<number[]>(
    Array(WAVEFORM_POINTS).fill(0),
  );

  // Refs
  const waveformTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  const stereoTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  const dialOffset = useRef(new Animated.Value(0)).current;

  // ── Waveform polling ────────────────────────────────────────────────────────

  const startWaveformPolling = useCallback(() => {
    waveformTimer.current = setInterval(async () => {
      try {
        const data = await SdrModule.getWaveformBuffer();
        if (data && data.length > 0) {
          setWaveformData(data);
        }
      } catch (_) {}
    }, 16); // ~60fps
  }, []);

  const stopWaveformPolling = useCallback(() => {
    if (waveformTimer.current) {
      clearInterval(waveformTimer.current);
      waveformTimer.current = null;
    }
  }, []);

  // ── Stereo detection polling ────────────────────────────────────────────────

  useEffect(() => {
    if (!isPlaying) return;
    stereoTimer.current = setInterval(async () => {
      try {
        const stereo = await SdrModule.checkStereo();
        setIsStereo(stereo);
      } catch (_) {}
    }, 1000);
    return () => {
      if (stereoTimer.current) clearInterval(stereoTimer.current);
    };
  }, [isPlaying]);

  // ── Playback ────────────────────────────────────────────────────────────────

  const handlePlayStop = async () => {
    if (isPlaying) {
      await SdrModule.stopFm();
      setIsPlaying(false);
      setIsStereo(false);
      stopWaveformPolling();
      setWaveformData(Array(WAVEFORM_POINTS).fill(0));
    } else {
      try {
        const fd = await SdrModule.requestUsbPermission();
        await SdrModule.startFm(fd, frequency * 1e6, 96000, true);
        setIsPlaying(true);
        startWaveformPolling();
      } catch (e: any) {
        console.error("FM start error:", e.message);
      }
    }
  };

  const tune = async (freq: number) => {
    const clamped = Math.max(
      FM_MIN,
      Math.min(FM_MAX, Math.round(freq * 10) / 10),
    );
    setFrequency(clamped);
    if (isPlaying) {
      await SdrModule.tuneFrequency(clamped * 1e6);
    }
  };

  // ── Dial pan responder ──────────────────────────────────────────────────────

  const dialStart = useRef(0);
  const freqAtDragStart = useRef(frequency);

  const panResponder = PanResponder.create({
    onStartShouldSetPanResponder: () => true,
    onPanResponderGrant: () => {
      freqAtDragStart.current = frequency;
      dialStart.current = 0;
    },
    onPanResponderMove: (_, gs) => {
      // 1px = 0.01 MHz, drag left = lower frequency
      const delta = -(gs.dx / 100) * FM_STEP * 10;
      tune(freqAtDragStart.current + delta);
    },
  });

  // ── Keypad input ────────────────────────────────────────────────────────────

  const handleKeypadSubmit = () => {
    const parsed = parseFloat(keypadInput);
    if (!isNaN(parsed)) {
      tune(parsed);
    }
    setKeypadInput("");
  };

  // ── Waveform path ───────────────────────────────────────────────────────────

  const buildWaveformPath = useCallback((): string => {
    if (waveformData.every((v) => v === 0)) {
      // Flat line when no signal
      return `M 0 ${WAVEFORM_HEIGHT / 2} L ${WAVEFORM_WIDTH} ${WAVEFORM_HEIGHT / 2}`;
    }

    const mid = WAVEFORM_HEIGHT / 2;
    const points = waveformData;
    const xStep = WAVEFORM_WIDTH / (points.length - 1);

    if (waveformMode === "oscilloscope") {
      // Raw waveform — every sample plotted
      let path = `M 0 ${mid + points[0] * mid}`;
      for (let i = 1; i < points.length; i++) {
        const x = i * xStep;
        const y = mid + points[i] * mid;
        path += ` L ${x} ${y}`;
      }
      return path;
    } else {
      // Envelope — smoothed amplitude, mirrored top and bottom
      const windowSize = 16;
      const envelopeTop: number[] = [];
      const envelopeBottom: number[] = [];

      for (let i = 0; i < points.length; i++) {
        const start = Math.max(0, i - windowSize);
        const end = Math.min(points.length - 1, i + windowSize);
        let max = 0;
        for (let j = start; j <= end; j++) {
          max = Math.max(max, Math.abs(points[j]));
        }
        envelopeTop.push(mid - max * mid * 0.9);
        envelopeBottom.push(mid + max * mid * 0.9);
      }

      // Draw filled envelope shape
      let path = `M 0 ${mid}`;
      for (let i = 0; i < envelopeTop.length; i++) {
        path += ` L ${i * xStep} ${envelopeTop[i]}`;
      }
      for (let i = envelopeTop.length - 1; i >= 0; i--) {
        path += ` L ${i * xStep} ${envelopeBottom[i]}`;
      }
      path += " Z";
      return path;
    }
  }, [waveformData, waveformMode]);

  // ── Preset management ───────────────────────────────────────────────────────

  const savePreset = () => {
    const exists = presets.find((p) => p.frequency === frequency);
    if (exists) return;
    setPresets((prev) =>
      [...prev, { frequency, label: `${frequency.toFixed(1)} FM` }].sort(
        (a, b) => a.frequency - b.frequency,
      ),
    );
  };

  const removePreset = (freq: number) => {
    setPresets((prev) => prev.filter((p) => p.frequency !== freq));
  };

  // ── Cleanup ─────────────────────────────────────────────────────────────────

  useEffect(() => {
    return () => {
      stopWaveformPolling();
      if (isPlaying) SdrModule.stopFm();
    };
  }, []);

  // ── Render ──────────────────────────────────────────────────────────────────

  const waveformPath = buildWaveformPath();

  return (
    <View style={s.container}>
      {/* ── RDS Info ── */}
      <View style={s.rdsBar}>
        <Text style={s.rdsStation}>
          {rdsStation || (isPlaying ? "··· scanning ···" : "SDRGo FM")}
        </Text>
        <Text style={s.rdsText} numberOfLines={1}>
          {rdsText || ""}
        </Text>
        <View style={s.rdsRight}>
          {isStereo && (
            <View style={[s.stereoBadge, { backgroundColor: theme.success }]}>
              <Text style={s.stereoBadgeText}>STEREO</Text>
            </View>
          )}
        </View>
      </View>

      {/* ── Waveform ── */}
      <View style={s.waveformContainer}>
        <Canvas style={{ width: WAVEFORM_WIDTH, height: WAVEFORM_HEIGHT }}>
          <Path
            path={waveformPath}
            color={
              waveformMode === "envelope" ? theme.primaryDim : theme.primary
            }
            style={waveformMode === "envelope" ? "fill" : "stroke"}
            strokeWidth={1.5}
          />
          {waveformMode === "oscilloscope" && (
            <Path
              path={`M 0 ${WAVEFORM_HEIGHT / 2} L ${WAVEFORM_WIDTH} ${WAVEFORM_HEIGHT / 2}`}
              color={theme.border}
              style="stroke"
              strokeWidth={0.5}
            />
          )}
        </Canvas>

        {/* Waveform mode toggle */}
        <View style={s.waveformToggle}>
          {(["oscilloscope", "envelope"] as WaveformMode[]).map((mode) => (
            <TouchableOpacity
              key={mode}
              style={[
                s.toggleBtn,
                waveformMode === mode && { backgroundColor: theme.primaryDim },
              ]}
              onPress={() => setWaveformMode(mode)}
            >
              <Text
                style={[
                  s.toggleBtnText,
                  {
                    color:
                      waveformMode === mode
                        ? theme.primary
                        : theme.textSecondary,
                  },
                ]}
              >
                {mode === "oscilloscope" ? "SCOPE" : "ENV"}
              </Text>
            </TouchableOpacity>
          ))}
        </View>
      </View>

      {/* ── Frequency Display ── */}
      <View style={s.freqDisplay}>
        <Text style={s.freqValue}>{frequency.toFixed(1)}</Text>
        <Text style={s.freqUnit}>MHz</Text>
      </View>

      {/* ── Tuning mode toggle ── */}
      <View style={s.tuningToggle}>
        {(["dial", "keypad"] as TuningMode[]).map((mode) => (
          <TouchableOpacity
            key={mode}
            style={[
              s.tuningToggleBtn,
              tuningMode === mode && { borderColor: theme.primary },
            ]}
            onPress={() => setTuningMode(mode)}
          >
            <Text
              style={[
                s.tuningToggleBtnText,
                {
                  color:
                    tuningMode === mode ? theme.primary : theme.textSecondary,
                },
              ]}
            >
              {mode === "dial" ? "⟵  DIAL  ⟶" : "⌨  KEYPAD"}
            </Text>
          </TouchableOpacity>
        ))}
      </View>

      {/* ── Tuning input ── */}
      {tuningMode === "dial" ? (
        <View style={s.dialArea} {...panResponder.panHandlers}>
          <View style={s.dialTrack}>
            {Array.from({ length: 21 }).map((_, i) => (
              <View
                key={i}
                style={[
                  s.dialTick,
                  i === 10 && s.dialTickCenter,
                  { backgroundColor: i === 10 ? theme.primary : theme.border },
                ]}
              />
            ))}
          </View>
          <Text style={[s.dialHint, { color: theme.textSecondary }]}>
            ← drag to tune →
          </Text>
          <View style={s.dialStepButtons}>
            <TouchableOpacity
              style={s.stepBtn}
              onPress={() => tune(frequency - FM_STEP)}
            >
              <Text style={[s.stepBtnText, { color: theme.primary }]}>
                −0.1
              </Text>
            </TouchableOpacity>
            <TouchableOpacity
              style={s.stepBtn}
              onPress={() => tune(frequency + FM_STEP)}
            >
              <Text style={[s.stepBtnText, { color: theme.primary }]}>
                +0.1
              </Text>
            </TouchableOpacity>
          </View>
        </View>
      ) : (
        <View style={s.keypadArea}>
          <TextInput
            style={[
              s.keypadInput,
              {
                color: theme.text,
                borderColor: theme.border,
                backgroundColor: theme.surface,
              },
            ]}
            value={keypadInput}
            onChangeText={setKeypadInput}
            keyboardType="decimal-pad"
            placeholder={`${frequency.toFixed(1)}`}
            placeholderTextColor={theme.textSecondary}
            onSubmitEditing={handleKeypadSubmit}
            returnKeyType="go"
          />
          <TouchableOpacity
            style={[s.keypadGoBtn, { backgroundColor: theme.primary }]}
            onPress={handleKeypadSubmit}
          >
            <Text style={s.keypadGoBtnText}>TUNE</Text>
          </TouchableOpacity>
        </View>
      )}

      {/* ── Play / Stop ── */}
      <TouchableOpacity
        style={[
          s.playBtn,
          { backgroundColor: isPlaying ? theme.accent : theme.primary },
        ]}
        onPress={handlePlayStop}
      >
        <Text style={s.playBtnText}>{isPlaying ? "■  STOP" : "▶  PLAY"}</Text>
      </TouchableOpacity>

      {/* ── Presets ── */}
      <View style={s.presetsHeader}>
        <Text style={[s.presetsTitle, { color: theme.textSecondary }]}>
          PRESETS
        </Text>
        <TouchableOpacity onPress={savePreset}>
          <Text style={[s.saveBtn, { color: theme.primary }]}>+ SAVE</Text>
        </TouchableOpacity>
      </View>

      <ScrollView
        horizontal
        showsHorizontalScrollIndicator={false}
        contentContainerStyle={s.presetsList}
      >
        {presets.map((preset) => (
          <TouchableOpacity
            key={preset.frequency}
            style={[
              s.presetChip,
              {
                backgroundColor:
                  frequency === preset.frequency
                    ? theme.primaryDim
                    : theme.surface,
                borderColor:
                  frequency === preset.frequency ? theme.primary : theme.border,
              },
            ]}
            onPress={() => tune(preset.frequency)}
            onLongPress={() => removePreset(preset.frequency)}
          >
            <Text
              style={[
                s.presetFreq,
                {
                  color:
                    frequency === preset.frequency ? theme.primary : theme.text,
                },
              ]}
            >
              {preset.frequency.toFixed(1)}
            </Text>
            <Text style={[s.presetLabel, { color: theme.textSecondary }]}>
              {preset.label}
            </Text>
          </TouchableOpacity>
        ))}
      </ScrollView>
    </View>
  );
}

const styles = (theme: any) =>
  StyleSheet.create({
    container: {
      flex: 1,
      backgroundColor: theme.background,
      paddingHorizontal: 16,
    },
    // RDS
    rdsBar: {
      flexDirection: "row",
      alignItems: "center",
      paddingVertical: 12,
      borderBottomWidth: 1,
      borderBottomColor: theme.border,
      gap: 8,
    },
    rdsStation: {
      fontSize: 14,
      fontWeight: "700",
      color: theme.text,
      minWidth: 80,
    },
    rdsText: {
      flex: 1,
      fontSize: 12,
      color: theme.textSecondary,
      fontFamily: "monospace",
    },
    rdsRight: {
      alignItems: "flex-end",
    },
    stereoBadge: {
      paddingHorizontal: 8,
      paddingVertical: 3,
      borderRadius: 4,
    },
    stereoBadgeText: {
      fontSize: 10,
      fontWeight: "800",
      color: "#000",
      letterSpacing: 1,
    },
    // Waveform
    waveformContainer: {
      marginTop: 12,
      marginBottom: 4,
      alignItems: "center",
      backgroundColor: theme.surface,
      borderRadius: 12,
      padding: 8,
      borderWidth: 1,
      borderColor: theme.border,
    },
    waveformToggle: {
      flexDirection: "row",
      gap: 8,
      marginTop: 6,
    },
    toggleBtn: {
      paddingHorizontal: 12,
      paddingVertical: 4,
      borderRadius: 6,
    },
    toggleBtnText: {
      fontSize: 10,
      fontWeight: "700",
      letterSpacing: 1,
    },
    // Frequency
    freqDisplay: {
      flexDirection: "row",
      alignItems: "baseline",
      justifyContent: "center",
      marginVertical: 16,
    },
    freqValue: {
      fontSize: 56,
      fontWeight: "800",
      color: theme.primary,
      fontFamily: "monospace",
      letterSpacing: -2,
    },
    freqUnit: {
      fontSize: 20,
      color: theme.textSecondary,
      marginLeft: 8,
      fontWeight: "600",
    },
    // Tuning toggle
    tuningToggle: {
      flexDirection: "row",
      gap: 8,
      justifyContent: "center",
      marginBottom: 12,
    },
    tuningToggleBtn: {
      paddingHorizontal: 16,
      paddingVertical: 8,
      borderRadius: 8,
      borderWidth: 1,
      borderColor: theme.border,
    },
    tuningToggleBtnText: {
      fontSize: 12,
      fontWeight: "700",
      letterSpacing: 1,
    },
    // Dial
    dialArea: {
      alignItems: "center",
      paddingVertical: 8,
    },
    dialTrack: {
      flexDirection: "row",
      alignItems: "flex-end",
      gap: 6,
      height: 32,
      paddingHorizontal: 8,
    },
    dialTick: {
      width: 2,
      height: 16,
      borderRadius: 1,
    },
    dialTickCenter: {
      height: 28,
      width: 2,
    },
    dialHint: {
      fontSize: 11,
      marginTop: 6,
      letterSpacing: 1,
    },
    dialStepButtons: {
      flexDirection: "row",
      gap: 24,
      marginTop: 10,
    },
    stepBtn: {
      paddingHorizontal: 20,
      paddingVertical: 8,
      borderRadius: 8,
      borderWidth: 1,
      borderColor: theme.border,
    },
    stepBtnText: {
      fontSize: 14,
      fontWeight: "700",
      fontFamily: "monospace",
    },
    // Keypad
    keypadArea: {
      flexDirection: "row",
      gap: 8,
      alignItems: "center",
      paddingVertical: 8,
    },
    keypadInput: {
      flex: 1,
      height: 48,
      borderWidth: 1,
      borderRadius: 10,
      paddingHorizontal: 16,
      fontSize: 20,
      fontFamily: "monospace",
      fontWeight: "700",
      textAlign: "center",
    },
    keypadGoBtn: {
      height: 48,
      paddingHorizontal: 20,
      borderRadius: 10,
      justifyContent: "center",
      alignItems: "center",
    },
    keypadGoBtnText: {
      fontSize: 14,
      fontWeight: "800",
      color: "#000",
      letterSpacing: 1,
    },
    // Play button
    playBtn: {
      height: 52,
      borderRadius: 12,
      justifyContent: "center",
      alignItems: "center",
      marginVertical: 12,
    },
    playBtnText: {
      fontSize: 16,
      fontWeight: "800",
      color: "#000",
      letterSpacing: 2,
    },
    // Presets
    presetsHeader: {
      flexDirection: "row",
      justifyContent: "space-between",
      alignItems: "center",
      marginBottom: 8,
    },
    presetsTitle: {
      fontSize: 11,
      fontWeight: "700",
      letterSpacing: 2,
    },
    saveBtn: {
      fontSize: 12,
      fontWeight: "700",
      letterSpacing: 1,
    },
    presetsList: {
      gap: 8,
      paddingBottom: 16,
    },
    presetChip: {
      paddingHorizontal: 14,
      paddingVertical: 10,
      borderRadius: 10,
      borderWidth: 1,
      alignItems: "center",
      minWidth: 80,
    },
    presetFreq: {
      fontSize: 15,
      fontWeight: "700",
      fontFamily: "monospace",
    },
    presetLabel: {
      fontSize: 10,
      marginTop: 2,
    },
  });

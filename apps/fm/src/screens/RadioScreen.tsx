/**
 * RadioScreen.tsx
 * SDRGo FM/AM Radio — single-page immersive interface
 *
 * Layout (top to bottom):
 *   ┌──────────────────────────────────┐
 *   │  [gear]    SDRGo FM     [mute]   │  ← top bar
 *   │──────────────────────────────────│
 *   │  RDS Programme Service + Text    │  ← RDS bar
 *   │──────────────────────────────────│
 *   │                                  │
 *   │   WAVEFORM VISUAL (full-width)   │  ← Skia canvas, behind freq
 *   │                                  │
 *   │       FM ●  AM          STEREO   │  ← band toggle + stereo badge
 *   │   ○  88 . 5  MHz                │  ← frequency (tap to type)
 *   │   ▁▃▅▅▃▁  signal strength        │  ← signal bars
 *   │                                  │
 *   ├──────────────────────────────────┤
 *   │  [◀◀]   [⏺ REC]  [▶▶]           │  ← transport bar
 *   ├──────────────────────────────────┤
 *   │  ╔════════ DIAL ════════╗        │  ← rotary dial (docked bottom)
 *   │  ║   ∣ ∣ ∣║∣ ∣ ∣       ║        │
 *   │  ╚══════════════════════╝        │
 *   └──────────────────────────────────┘
 */

import React, {
  useState,
  useEffect,
  useRef,
  useCallback,
  useMemo,
} from "react";
import {
  View,
  Text,
  StyleSheet,
  TouchableOpacity,
  TextInput,
  StatusBar,
  Dimensions,
  Platform,
  AppState,
  Modal,
  ScrollView,
  Pressable,
  Animated,
  PanResponder,
} from "react-native";
import {
  SafeAreaView,
  useSafeAreaInsets,
} from "react-native-safe-area-context";
import {
  Canvas,
  Path,
  Paint,
  LinearGradient,
  vec,
  Rect,
} from "@shopify/react-native-skia";
import { useTheme, SdrModule, usePresets } from "@sdrgo/ui-core";
import {
  getRdsStationInfo,
  getSignalInfo,
  getHardwareGainInfo,
  setHardwareGain,
  setEq,
  setMonoMode,
  scan,
  startRecording,
  stopRecording,
  EQ_BANDS,
  DEFAULT_EQ,
  DEFAULT_GAIN_TENTHS,
  type RdsStationInfo,
  type SignalInfo,
} from "@sdrgo/ui-core";

import EqSheet from "../components/EqSheet";
import SettingsScreen from "./SettingsScreen";
import RotaryDial from "../components/RotaryDial";
import WaveformVisual from "../components/WaveformVisual";
import SignalMeter from "../components/SignalMeter";
import Toast from "../components/Toast";

const { width: SCREEN_W, height: SCREEN_H } = Dimensions.get("window");

// ── Band constants ────────────────────────────────────────────────────────────
const FM_MIN = 87.5e6;
const FM_MAX = 108.0e6;
const FM_STEP = 0.1e6;

const AM_MIN = 520e3;
const AM_MAX = 1710e3;
const AM_STEP = 10e3;

// ── Helpers ───────────────────────────────────────────────────────────────────
function formatFrequency(hz: number, band: "fm" | "am"): string {
  if (band === "fm") {
    return (hz / 1e6).toFixed(1);
  }
  return Math.round(hz / 1e3).toString();
}

function clampFrequency(hz: number, band: "fm" | "am"): number {
  const min = band === "fm" ? FM_MIN : AM_MIN;
  const max = band === "fm" ? FM_MAX : AM_MAX;
  const step = band === "fm" ? FM_STEP : AM_STEP;
  const clamped = Math.max(min, Math.min(max, hz));
  return Math.round(clamped / step) * step;
}

function wrapFrequency(hz: number, band: "fm" | "am"): number {
  const min = band === "fm" ? FM_MIN : AM_MIN;
  const max = band === "fm" ? FM_MAX : AM_MAX;
  if (hz > max) return min;
  if (hz < min) return max;
  return hz;
}

function makeRecordingName(
  hz: number,
  band: "fm" | "am",
  rdsPs: string | null,
): string {
  const ts = new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19);
  const label =
    rdsPs?.trim() ||
    formatFrequency(hz, band) + (band === "fm" ? "MHz" : "kHz");
  return `${ts}_${label}`;
}

// ── Component ─────────────────────────────────────────────────────────────────
export default function RadioScreen() {
  const { theme, isDark } = useTheme();
  const insets = useSafeAreaInsets();
  const s = useMemo(() => styles(theme, insets), [theme, insets]);

  // ── State ──────────────────────────────────────────────────────────────────
  const [band, setBand] = useState<"fm" | "am">("fm");
  const [frequencyHz, setFrequencyHz] = useState(88.5e6);
  const [isPlaying, setIsPlaying] = useState(false);
  const [isMuted, setIsMuted] = useState(false);
  const [isRecording, setIsRecording] = useState(false);
  const [waveformData, setWaveformData] = useState<number[]>(
    Array(512).fill(0),
  );
  const [rdsInfo, setRdsInfo] = useState<RdsStationInfo | null>(null);
  const [signalInfo, setSignalInfo] = useState<SignalInfo | null>(null);
  const [showEqSheet, setShowEqSheet] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [freqInputMode, setFreqInputMode] = useState(false);
  const [freqInputText, setFreqInputText] = useState("");
  const [isScanning, setIsScanning] = useState(false);
  const [visualMode, setVisualMode] = useState<"artistic" | "oscilloscope">(
    "artistic",
  );
  const [toast, setToast] = useState<{ message: string; sub?: string } | null>(
    null,
  );

  // Per-session active settings (overridden by preset when loading one)
  const [gainTenthsDb, setGainTenthsDb] = useState(DEFAULT_GAIN_TENTHS);
  const [eq, setEqBands] = useState<number[]>([...DEFAULT_EQ]);
  const [forceMono, setForceMono] = useState(false);

  const { presets, savePreset, getPresetsForBand } = usePresets();

  // ── Refs ───────────────────────────────────────────────────────────────────
  const waveformTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  const rdsTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  const signalTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  const scanLongPressTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isScanningRef = useRef(false);
  const frequencyRef = useRef(frequencyHz);
  const bandRef = useRef(band);
  const appState = useRef(AppState.currentState);

  useEffect(() => {
    frequencyRef.current = frequencyHz;
  }, [frequencyHz]);
  useEffect(() => {
    bandRef.current = band;
  }, [band]);

  // ── App background / foreground ────────────────────────────────────────────
  useEffect(() => {
    const sub = AppState.addEventListener("change", (state) => {
      appState.current = state;
      // Audio continues in background (handled by native AudioTrack)
      // Just pause waveform polling to save CPU
      if (state === "background" || state === "inactive") {
        stopWaveformPolling();
      } else if (state === "active" && isPlaying) {
        startWaveformPolling();
      }
    });
    return () => sub.remove();
  }, [isPlaying]);

  // ── Waveform polling ───────────────────────────────────────────────────────
  const startWaveformPolling = useCallback(() => {
    if (waveformTimer.current) return;
    waveformTimer.current = setInterval(async () => {
      try {
        const data = await SdrModule.getWaveformBuffer();
        if (data && data.length > 0) setWaveformData(data);
      } catch (_) {}
    }, 16);
  }, []);

  const stopWaveformPolling = useCallback(() => {
    if (waveformTimer.current) {
      clearInterval(waveformTimer.current);
      waveformTimer.current = null;
    }
  }, []);

  // ── RDS polling ────────────────────────────────────────────────────────────
  useEffect(() => {
    rdsTimer.current = setInterval(async () => {
      const info = await getRdsStationInfo(
        frequencyRef.current,
        bandRef.current,
      );
      setRdsInfo(info);
    }, 2000);
    return () => {
      if (rdsTimer.current) clearInterval(rdsTimer.current);
    };
  }, []);

  // ── Signal polling ─────────────────────────────────────────────────────────
  useEffect(() => {
    signalTimer.current = setInterval(async () => {
      const info = await getSignalInfo(frequencyRef.current, bandRef.current);
      setSignalInfo(info);
    }, 500);
    return () => {
      if (signalTimer.current) clearInterval(signalTimer.current);
    };
  }, []);

  // ── Playback ───────────────────────────────────────────────────────────────
  const handlePlayToggle = useCallback(async () => {
    if (isPlaying) {
      await SdrModule.stopFm();
      setIsPlaying(false);
      stopWaveformPolling();
      setWaveformData(Array(512).fill(0));
    } else {
      try {
        if (band === "am") {
          // AM: no audio yet, but start UI flow
          setIsPlaying(true);
          showToast("AM audio coming soon", "Signal controls are active");
          return;
        }
        const fd = await SdrModule.requestUsbPermission();
        await SdrModule.startFm(fd, frequencyHz, 96000, !forceMono);
        setIsPlaying(true);
        startWaveformPolling();
      } catch (e: any) {
        showToast(
          "Could not start radio",
          e?.message ?? "Check USB connection",
        );
      }
    }
  }, [isPlaying, band, frequencyHz, forceMono]);

  // ── Tune ───────────────────────────────────────────────────────────────────
  const tune = useCallback(
    async (hz: number) => {
      const clamped = clampFrequency(hz, bandRef.current);
      setFrequencyHz(clamped);
      setRdsInfo(null); // clear old RDS
      if (isPlaying && bandRef.current === "fm") {
        await SdrModule.tuneFrequency(clamped);
      }
    },
    [isPlaying],
  );

  // ── Band toggle ────────────────────────────────────────────────────────────
  const handleBandToggle = useCallback(
    (newBand: "fm" | "am") => {
      if (newBand === band) return;
      if (isPlaying) SdrModule.stopFm();
      setIsPlaying(false);
      stopWaveformPolling();
      setWaveformData(Array(512).fill(0));
      setRdsInfo(null);
      setBand(newBand);
      setFrequencyHz(newBand === "fm" ? 88.5e6 : 1000e3);
    },
    [band, isPlaying],
  );

  // ── Frequency manual input ─────────────────────────────────────────────────
  const handleFreqTap = useCallback(() => {
    setFreqInputText(formatFrequency(frequencyHz, band));
    setFreqInputMode(true);
  }, [frequencyHz, band]);

  const handleFreqSubmit = useCallback(() => {
    const parsed = parseFloat(freqInputText);
    if (!isNaN(parsed)) {
      const hz = band === "fm" ? parsed * 1e6 : parsed * 1e3;
      tune(hz);
    }
    setFreqInputMode(false);
  }, [freqInputText, band, tune]);

  // ── Preset navigation (step) ───────────────────────────────────────────────
  const navigatePreset = useCallback(
    (direction: "prev" | "next") => {
      const bandPresets = getPresetsForBand(band);
      if (bandPresets.length === 0) return;
      const currentIdx = bandPresets.findIndex(
        (p) => p.frequencyHz === frequencyHz,
      );
      let nextIdx: number;
      if (currentIdx < 0) {
        nextIdx = direction === "next" ? 0 : bandPresets.length - 1;
      } else {
        nextIdx =
          direction === "next"
            ? (currentIdx + 1) % bandPresets.length
            : (currentIdx - 1 + bandPresets.length) % bandPresets.length;
      }
      const preset = bandPresets[nextIdx];
      tune(preset.frequencyHz);
      // Restore preset settings
      setGainTenthsDb(preset.gainTenthsDb);
      setEqBands([...preset.eq]);
      setForceMono(preset.mono);
      setHardwareGain(preset.gainTenthsDb);
      setEq(preset.eq);
      setMonoMode(preset.mono);
    },
    [band, frequencyHz, getPresetsForBand, tune],
  );

  // ── Scanning (long press) ──────────────────────────────────────────────────
  const startScan = useCallback(
    async (direction: "up" | "down") => {
      if (isScanningRef.current) return;
      isScanningRef.current = true;
      setIsScanning(true);
      try {
        const nextHz = await scan(
          frequencyRef.current,
          direction,
          bandRef.current,
        );
        if (isScanningRef.current) {
          const wrapped = wrapFrequency(nextHz, bandRef.current);
          await tune(wrapped);
          if (bandRef.current === "fm" && isPlaying) {
            await SdrModule.tuneFrequency(wrapped);
          }
        }
      } finally {
        isScanningRef.current = false;
        setIsScanning(false);
      }
    },
    [isPlaying, tune],
  );

  const cancelScan = useCallback(() => {
    isScanningRef.current = false;
    setIsScanning(false);
    if (scanLongPressTimer.current) {
      clearTimeout(scanLongPressTimer.current);
      scanLongPressTimer.current = null;
    }
  }, []);

  // ── Bookmark / save preset ─────────────────────────────────────────────────
  const handleBookmark = useCallback(() => {
    const ps = rdsInfo?.ps?.trim() || null;
    const name =
      ps ||
      formatFrequency(frequencyHz, band) + (band === "fm" ? " FM" : " AM");
    savePreset(frequencyHz, band, name, ps, gainTenthsDb, eq, forceMono);
    showToast(`Saved: ${name}`, "Manage in settings to rename or delete");
  }, [frequencyHz, band, rdsInfo, gainTenthsDb, eq, forceMono, savePreset]);

  // ── Recording ──────────────────────────────────────────────────────────────
  const handleRecordToggle = useCallback(async () => {
    if (isRecording) {
      const path = await stopRecording();
      setIsRecording(false);
      const filename = path.split("/").pop() ?? "recording";
      showToast(`Saved: ${filename}`, "Find it in Settings → Recordings");
    } else {
      const name = makeRecordingName(frequencyHz, band, rdsInfo?.ps ?? null);
      await startRecording(name);
      setIsRecording(true);
      showToast(`Recording started`, name);
    }
  }, [isRecording, frequencyHz, band, rdsInfo]);

  // ── Mute ──────────────────────────────────────────────────────────────────
  const handleMuteToggle = useCallback(() => {
    setIsMuted((prev) => !prev);
    // REAL: SdrModule.setMute(!isMuted)
  }, []);

  // ── EQ/gain changes ────────────────────────────────────────────────────────
  const handleEqChange = useCallback((bands: number[]) => {
    setEqBands(bands);
    setEq(bands);
  }, []);

  const handleGainChange = useCallback((tenthsDb: number) => {
    setGainTenthsDb(tenthsDb);
    setHardwareGain(tenthsDb);
  }, []);

  const handleMonoChange = useCallback(
    (mono: boolean) => {
      setForceMono(mono);
      setMonoMode(mono);
      if (isPlaying && band === "fm") {
        // Retune to apply mono/stereo change
        SdrModule.tuneFrequency(frequencyHz);
      }
    },
    [isPlaying, band, frequencyHz],
  );

  // ── Toast helper ──────────────────────────────────────────────────────────
  const showToast = (message: string, sub?: string) => {
    setToast({ message, sub });
    setTimeout(() => setToast(null), 3500);
  };

  // ── Cleanup ────────────────────────────────────────────────────────────────
  useEffect(() => {
    return () => {
      stopWaveformPolling();
      if (rdsTimer.current) clearInterval(rdsTimer.current);
      if (signalTimer.current) clearInterval(signalTimer.current);
      if (isPlaying) SdrModule.stopFm();
    };
  }, []);

  // ── Derived display values ─────────────────────────────────────────────────
  const freqDisplay = formatFrequency(frequencyHz, band);
  const freqUnit = band === "fm" ? "MHz" : "kHz";
  const isStereo = signalInfo?.stereo && !forceMono;
  const isBookmarked = !!presets.find(
    (p) => p.frequencyHz === frequencyHz && p.band === band,
  );

  // ── Render ─────────────────────────────────────────────────────────────────
  return (
    <View style={[s.root, { backgroundColor: theme.background }]}>
      <StatusBar
        barStyle={isDark ? "light-content" : "dark-content"}
        translucent
        backgroundColor="transparent"
      />

      {/* ── Full-bleed waveform behind everything ── */}
      <View style={StyleSheet.absoluteFill} pointerEvents="none">
        <WaveformVisual
          data={waveformData}
          mode={visualMode}
          theme={theme}
          isPlaying={isPlaying}
          width={SCREEN_W}
          height={SCREEN_H}
        />
      </View>

      <SafeAreaView style={s.safeArea} edges={["top"]}>
        {/* ── Top bar ── */}
        <View style={s.topBar}>
          <TouchableOpacity
            style={s.iconBtn}
            onPress={() => setShowSettings(true)}
            hitSlop={{ top: 12, bottom: 12, left: 12, right: 12 }}
          >
            <Text style={[s.iconBtnText, { color: theme.textSecondary }]}>
              ⚙
            </Text>
          </TouchableOpacity>

          <Text style={[s.appTitle, { color: theme.primary }]}>SDRGo</Text>

          <View style={s.topRight}>
            <TouchableOpacity
              style={s.iconBtn}
              onPress={handleMuteToggle}
              hitSlop={{ top: 12, bottom: 12, left: 12, right: 12 }}
            >
              <Text
                style={[
                  s.iconBtnText,
                  { color: isMuted ? theme.danger : theme.textSecondary },
                ]}
              >
                {isMuted ? "🔇" : "🔊"}
              </Text>
            </TouchableOpacity>
            <TouchableOpacity
              style={s.iconBtn}
              onPress={handleBookmark}
              hitSlop={{ top: 12, bottom: 12, left: 12, right: 12 }}
            >
              <Text
                style={[
                  s.iconBtnText,
                  { color: isBookmarked ? theme.primary : theme.textSecondary },
                ]}
              >
                {isBookmarked ? "★" : "☆"}
              </Text>
            </TouchableOpacity>
          </View>
        </View>

        {/* ── RDS bar ── */}
        <View style={s.rdsBar}>
          <Text style={[s.rdsPs, { color: theme.primary }]} numberOfLines={1}>
            {rdsInfo?.ps?.trim() || (isPlaying ? "— scanning —" : "——")}
          </Text>
          <Text
            style={[s.rdsRt, { color: theme.textSecondary }]}
            numberOfLines={1}
          >
            {rdsInfo?.rt ||
              (rdsInfo?.ptyName ? `Genre: ${rdsInfo.ptyName}` : "")}
          </Text>
        </View>

        {/* ── Main center area ── */}
        <View style={s.centerArea}>
          {/* Band toggle + stereo badge */}
          <View style={s.bandRow}>
            <View style={s.bandToggle}>
              {(["fm", "am"] as const).map((b) => (
                <TouchableOpacity
                  key={b}
                  style={[
                    s.bandBtn,
                    band === b && {
                      backgroundColor: theme.primaryGlow,
                      borderColor: theme.primary,
                    },
                  ]}
                  onPress={() => handleBandToggle(b)}
                >
                  <Text
                    style={[
                      s.bandBtnText,
                      { color: band === b ? theme.primary : theme.textDim },
                    ]}
                  >
                    {b.toUpperCase()}
                  </Text>
                </TouchableOpacity>
              ))}
            </View>

            {isStereo !== undefined && (
              <View
                style={[
                  s.stereoBadge,
                  {
                    backgroundColor: isStereo
                      ? theme.signalDim
                      : theme.primaryDim,
                  },
                ]}
              >
                <Text
                  style={[
                    s.stereoBadgeText,
                    { color: isStereo ? theme.signal : theme.primary },
                  ]}
                >
                  {isStereo ? "STEREO" : "MONO"}
                </Text>
              </View>
            )}
          </View>

          {/* Frequency display */}
          <TouchableOpacity onPress={handleFreqTap} activeOpacity={0.8}>
            {freqInputMode ? (
              <View style={s.freqInputRow}>
                <TextInput
                  style={[
                    s.freqInput,
                    { color: theme.primary, borderColor: theme.primary },
                  ]}
                  value={freqInputText}
                  onChangeText={setFreqInputText}
                  keyboardType="decimal-pad"
                  autoFocus
                  onSubmitEditing={handleFreqSubmit}
                  onBlur={handleFreqSubmit}
                  returnKeyType="go"
                  selectTextOnFocus
                />
                <Text style={[s.freqUnitLarge, { color: theme.textSecondary }]}>
                  {freqUnit}
                </Text>
              </View>
            ) : (
              <View style={s.freqRow}>
                <Text style={[s.freqValue, { color: theme.primary }]}>
                  {freqDisplay}
                </Text>
                <Text style={[s.freqUnit, { color: theme.textSecondary }]}>
                  {freqUnit}
                </Text>
              </View>
            )}
          </TouchableOpacity>

          {/* Signal meter */}
          <SignalMeter
            strength={signalInfo?.strength ?? 0}
            snr={signalInfo?.snr ?? 0}
            theme={theme}
          />
        </View>
      </SafeAreaView>

      {/* ── Transport bar ── */}
      <View style={[s.transportBar, { backgroundColor: theme.overlay }]}>
        {/* Previous / scan down */}
        <Pressable
          style={s.transportBtn}
          onPress={() => navigatePreset("prev")}
          onLongPress={() => startScan("down")}
          onPressOut={cancelScan}
          delayLongPress={500}
        >
          <Text
            style={[
              s.transportBtnText,
              {
                color: isScanning ? theme.primary : theme.textSecondary,
              },
            ]}
          >
            ◀◀
          </Text>
        </Pressable>

        {/* Record button */}
        <TouchableOpacity
          style={[
            s.recordBtn,
            { borderColor: isRecording ? theme.danger : theme.border },
          ]}
          onPress={handleRecordToggle}
        >
          <View
            style={[
              s.recordDot,
              { backgroundColor: isRecording ? theme.danger : theme.textDim },
              isRecording && s.recordDotActive,
            ]}
          />
        </TouchableOpacity>

        {/* Next / scan up */}
        <Pressable
          style={s.transportBtn}
          onPress={() => navigatePreset("next")}
          onLongPress={() => startScan("up")}
          onPressOut={cancelScan}
          delayLongPress={500}
        >
          <Text
            style={[
              s.transportBtnText,
              {
                color: isScanning ? theme.primary : theme.textSecondary,
              },
            ]}
          >
            ▶▶
          </Text>
        </Pressable>
      </View>

      {/* ── Rotary dial ── */}
      <View style={[s.dialContainer, { backgroundColor: theme.surface }]}>
        <RotaryDial
          frequencyHz={frequencyHz}
          band={band}
          onFrequencyChange={tune}
          theme={theme}
          isPlaying={isPlaying}
          onPlayToggle={handlePlayToggle}
          onEqPress={() => setShowEqSheet(true)}
        />
      </View>

      {/* ── EQ half-sheet ── */}
      <EqSheet
        visible={showEqSheet}
        onClose={() => setShowEqSheet(false)}
        eq={eq}
        onEqChange={handleEqChange}
        gainTenthsDb={gainTenthsDb}
        onGainChange={handleGainChange}
        isMono={forceMono}
        onMonoChange={handleMonoChange}
        isStereoAvailable={signalInfo?.stereo ?? false}
        theme={theme}
      />

      {/* ── Settings full-screen ── */}
      <Modal
        visible={showSettings}
        animationType="slide"
        presentationStyle="pageSheet"
        onRequestClose={() => setShowSettings(false)}
      >
        <SettingsScreen
          onClose={() => setShowSettings(false)}
          visualMode={visualMode}
          onVisualModeChange={setVisualMode}
        />
      </Modal>

      {/* ── Toast ── */}
      {toast && (
        <Toast
          message={toast.message}
          sub={toast.sub}
          theme={theme}
          insets={insets}
        />
      )}
    </View>
  );
}

// ── Styles ────────────────────────────────────────────────────────────────────
function styles(theme: any, insets: any) {
  const DIAL_HEIGHT = 220;
  const TRANSPORT_HEIGHT = 72;

  return StyleSheet.create({
    root: {
      flex: 1,
    },
    safeArea: {
      flex: 1,
    },

    // Top bar
    topBar: {
      flexDirection: "row",
      alignItems: "center",
      justifyContent: "space-between",
      paddingHorizontal: 20,
      paddingTop: 8,
      paddingBottom: 4,
    },
    appTitle: {
      fontSize: 18,
      fontWeight: "800",
      letterSpacing: 4,
    },
    topRight: {
      flexDirection: "row",
      gap: 4,
    },
    iconBtn: {
      width: 40,
      height: 40,
      alignItems: "center",
      justifyContent: "center",
    },
    iconBtnText: {
      fontSize: 20,
    },

    // RDS
    rdsBar: {
      paddingHorizontal: 24,
      paddingVertical: 6,
      borderBottomWidth: StyleSheet.hairlineWidth,
      borderBottomColor: theme.border,
      gap: 2,
    },
    rdsPs: {
      fontSize: 13,
      fontWeight: "700",
      letterSpacing: 2,
      fontFamily: Platform.select({ ios: "Courier New", android: "monospace" }),
    },
    rdsRt: {
      fontSize: 11,
      fontFamily: Platform.select({ ios: "Courier New", android: "monospace" }),
    },

    // Center
    centerArea: {
      flex: 1,
      justifyContent: "center",
      alignItems: "center",
      paddingHorizontal: 24,
      gap: 16,
    },

    // Band
    bandRow: {
      flexDirection: "row",
      alignItems: "center",
      justifyContent: "space-between",
      width: "100%",
    },
    bandToggle: {
      flexDirection: "row",
      backgroundColor: theme.surfaceRaised,
      borderRadius: 10,
      padding: 3,
      gap: 2,
    },
    bandBtn: {
      paddingHorizontal: 18,
      paddingVertical: 6,
      borderRadius: 8,
      borderWidth: 1,
      borderColor: "transparent",
    },
    bandBtnText: {
      fontSize: 13,
      fontWeight: "800",
      letterSpacing: 2,
    },
    stereoBadge: {
      paddingHorizontal: 10,
      paddingVertical: 5,
      borderRadius: 6,
    },
    stereoBadgeText: {
      fontSize: 10,
      fontWeight: "800",
      letterSpacing: 1.5,
    },

    // Frequency
    freqRow: {
      flexDirection: "row",
      alignItems: "baseline",
      gap: 8,
    },
    freqValue: {
      fontSize: 72,
      fontWeight: "300",
      letterSpacing: -2,
      fontFamily: Platform.select({
        ios: "Helvetica Neue",
        android: "sans-serif-light",
      }),
      includeFontPadding: false,
    },
    freqUnit: {
      fontSize: 22,
      fontWeight: "400",
      letterSpacing: 1,
      paddingBottom: 8,
    },
    freqInputRow: {
      flexDirection: "row",
      alignItems: "baseline",
      gap: 8,
    },
    freqInput: {
      fontSize: 64,
      fontWeight: "300",
      letterSpacing: -2,
      borderBottomWidth: 2,
      paddingVertical: 0,
      minWidth: 180,
      textAlign: "center",
      fontFamily: Platform.select({
        ios: "Helvetica Neue",
        android: "sans-serif-light",
      }),
    },
    freqUnitLarge: {
      fontSize: 22,
      fontWeight: "400",
      paddingBottom: 8,
    },

    // Transport
    transportBar: {
      flexDirection: "row",
      alignItems: "center",
      justifyContent: "space-around",
      height: TRANSPORT_HEIGHT,
      paddingHorizontal: 32,
      borderTopWidth: StyleSheet.hairlineWidth,
      borderTopColor: theme.border,
    },
    transportBtn: {
      width: 60,
      height: 48,
      alignItems: "center",
      justifyContent: "center",
    },
    transportBtnText: {
      fontSize: 22,
      fontWeight: "700",
      letterSpacing: 1,
    },
    recordBtn: {
      width: 56,
      height: 56,
      borderRadius: 28,
      borderWidth: 2,
      alignItems: "center",
      justifyContent: "center",
    },
    recordDot: {
      width: 20,
      height: 20,
      borderRadius: 10,
    },
    recordDotActive: {
      width: 12,
      height: 12,
      borderRadius: 2, // Square when recording
    },

    // Dial
    dialContainer: {
      height: DIAL_HEIGHT,
      borderTopWidth: StyleSheet.hairlineWidth,
      borderTopColor: theme.border,
    },
  });
}

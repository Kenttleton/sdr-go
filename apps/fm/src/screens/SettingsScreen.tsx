/**
 * SettingsScreen.tsx
 * Full settings page — launched as a modal sheet from the gear icon.
 *
 * Sections:
 *   - Saved Presets (rename, delete, view EQ/gain per preset)
 *   - Recordings (rename, delete, open file manager)
 *   - Visual Settings (waveform mode, theme)
 *   - About
 */

import React, { useState, useEffect, useCallback } from "react";
import {
  View,
  Text,
  StyleSheet,
  TouchableOpacity,
  ScrollView,
  TextInput,
  Alert,
  Platform,
  Linking,
} from "react-native";
import {
  SafeAreaView,
  useSafeAreaInsets,
} from "react-native-safe-area-context";
import { useTheme, usePresets } from "@sdrgo/ui-core";
import type { Preset } from "@sdrgo/ui-core";
import { useDevLogs, clearLogEntries } from "../dev/logger";

interface Props {
  onClose: () => void;
  visualMode: "artistic" | "oscilloscope";
  onVisualModeChange: (mode: "artistic" | "oscilloscope") => void;
}

// ── Preset row ────────────────────────────────────────────────────────────────
interface PresetRowProps {
  preset: Preset;
  onRename: (id: string, name: string) => void;
  onDelete: (id: string) => void;
  theme: ReturnType<typeof useTheme>["theme"];
}

function PresetRow({ preset, onRename, onDelete, theme }: PresetRowProps) {
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(preset.name);
  const [expanded, setExpanded] = useState(false);

  const freqLabel =
    preset.band === "fm"
      ? `${(preset.frequencyHz / 1e6).toFixed(1)} MHz FM`
      : `${Math.round(preset.frequencyHz / 1e3)} kHz AM`;

  const handleSave = () => {
    if (name.trim()) onRename(preset.id, name.trim());
    setEditing(false);
  };

  const handleDelete = () => {
    Alert.alert("Delete Preset", `Remove "${preset.name}"?`, [
      { text: "Cancel", style: "cancel" },
      {
        text: "Delete",
        style: "destructive",
        onPress: () => onDelete(preset.id),
      },
    ]);
  };

  return (
    <View
      style={[
        rowStyles.container,
        { backgroundColor: theme.surface, borderColor: theme.border },
      ]}
    >
      <TouchableOpacity
        style={rowStyles.header}
        onPress={() => setExpanded((e) => !e)}
        activeOpacity={0.7}
      >
        <View
          style={[
            rowStyles.bandTag,
            {
              backgroundColor:
                preset.band === "fm" ? theme.primaryDim : theme.signalDim,
            },
          ]}
        >
          <Text
            style={[
              rowStyles.bandTagText,
              {
                color: preset.band === "fm" ? theme.primary : theme.signal,
              },
            ]}
          >
            {preset.band.toUpperCase()}
          </Text>
        </View>

        <View style={rowStyles.nameCol}>
          {editing ? (
            <TextInput
              style={[
                rowStyles.nameInput,
                { color: theme.text, borderColor: theme.primary },
              ]}
              value={name}
              onChangeText={setName}
              autoFocus
              onSubmitEditing={handleSave}
              onBlur={handleSave}
              returnKeyType="done"
            />
          ) : (
            <Text style={[rowStyles.name, { color: theme.text }]}>
              {preset.name}
            </Text>
          )}
          <Text style={[rowStyles.freq, { color: theme.textSecondary }]}>
            {freqLabel}
          </Text>
        </View>

        <View style={rowStyles.actions}>
          <TouchableOpacity
            style={rowStyles.actionBtn}
            onPress={() => setEditing((e) => !e)}
          >
            <Text
              style={[rowStyles.actionBtnText, { color: theme.textSecondary }]}
            >
              ✎
            </Text>
          </TouchableOpacity>
          <TouchableOpacity style={rowStyles.actionBtn} onPress={handleDelete}>
            <Text style={[rowStyles.actionBtnText, { color: theme.danger }]}>
              ✕
            </Text>
          </TouchableOpacity>
          <Text style={[rowStyles.chevron, { color: theme.textDim }]}>
            {expanded ? "▲" : "▼"}
          </Text>
        </View>
      </TouchableOpacity>

      {expanded && (
        <View style={[rowStyles.details, { borderTopColor: theme.border }]}>
          <View style={rowStyles.detailRow}>
            <Text style={[rowStyles.detailLabel, { color: theme.textDim }]}>
              GAIN
            </Text>
            <Text
              style={[rowStyles.detailValue, { color: theme.textSecondary }]}
            >
              {(preset.gainTenthsDb / 10).toFixed(1)} dB
            </Text>
          </View>
          <View style={rowStyles.detailRow}>
            <Text style={[rowStyles.detailLabel, { color: theme.textDim }]}>
              MODE
            </Text>
            <Text
              style={[rowStyles.detailValue, { color: theme.textSecondary }]}
            >
              {preset.mono ? "Mono" : "Stereo"}
            </Text>
          </View>
          <View style={rowStyles.detailRow}>
            <Text style={[rowStyles.detailLabel, { color: theme.textDim }]}>
              EQ
            </Text>
            <Text
              style={[rowStyles.detailValue, { color: theme.textSecondary }]}
            >
              {preset.eq
                .map((v) => (v >= 0 ? "+" : "") + v.toFixed(1))
                .join("  ")}{" "}
              dB
            </Text>
          </View>
          <View style={rowStyles.detailRow}>
            <Text style={[rowStyles.detailLabel, { color: theme.textDim }]}>
              SAVED
            </Text>
            <Text
              style={[rowStyles.detailValue, { color: theme.textSecondary }]}
            >
              {new Date(preset.savedAt).toLocaleDateString()}
            </Text>
          </View>
        </View>
      )}
    </View>
  );
}

const rowStyles = StyleSheet.create({
  container: {
    borderRadius: 12,
    borderWidth: StyleSheet.hairlineWidth,
    overflow: "hidden",
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    padding: 14,
    gap: 10,
  },
  bandTag: {
    paddingHorizontal: 8,
    paddingVertical: 3,
    borderRadius: 5,
  },
  bandTagText: {
    fontSize: 9,
    fontWeight: "800",
    letterSpacing: 1.5,
  },
  nameCol: {
    flex: 1,
    gap: 2,
  },
  name: {
    fontSize: 14,
    fontWeight: "700",
  },
  nameInput: {
    fontSize: 14,
    fontWeight: "700",
    borderBottomWidth: 1,
    paddingVertical: 0,
  },
  freq: {
    fontSize: 11,
    fontFamily: "monospace",
  },
  actions: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
  },
  actionBtn: {
    width: 28,
    height: 28,
    alignItems: "center",
    justifyContent: "center",
  },
  actionBtnText: {
    fontSize: 16,
  },
  chevron: {
    fontSize: 10,
    marginLeft: 4,
  },
  details: {
    borderTopWidth: StyleSheet.hairlineWidth,
    padding: 14,
    gap: 8,
  },
  detailRow: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "baseline",
  },
  detailLabel: {
    fontSize: 9,
    fontWeight: "800",
    letterSpacing: 2,
    width: 44,
  },
  detailValue: {
    fontSize: 12,
    fontFamily: "monospace",
    flex: 1,
    textAlign: "right",
  },
});

// ── Main settings screen ──────────────────────────────────────────────────────

export default function SettingsScreen({
  onClose,
  visualMode,
  onVisualModeChange,
}: Props) {
  const { theme, mode: themeMode, setMode: setThemeMode, isDark } = useTheme();
  const insets = useSafeAreaInsets();
  const { presets, updatePreset, deletePreset, getPresetsForBand } =
    usePresets();

  const [activeTab, setActiveTab] = useState<
    "presets" | "recordings" | "display" | "about" | "logs"
  >("presets");

  const { entries: logEntries } = useDevLogs();

  const fmPresets = getPresetsForBand("fm");
  const amPresets = getPresetsForBand("am");

  const handleOpenRecordingsFolder = () => {
    // On Android, open the recordings folder via file manager intent
    // REAL: NativeModules.SdrModule.openRecordingsFolder()
    Alert.alert(
      "Recordings",
      "Recordings folder: /storage/emulated/0/SDRGo/Recordings\n\nOpen in Files app to manage recordings.",
      [{ text: "OK" }],
    );
  };

  const s = styles(theme, insets);

  return (
    <View style={[s.root, { backgroundColor: theme.background }]}>
      {/* Header */}
      <SafeAreaView edges={["top"]} style={s.headerWrap}>
        <View style={s.header}>
          <Text style={[s.title, { color: theme.text }]}>Settings</Text>
          <TouchableOpacity onPress={onClose} style={s.closeBtn}>
            <Text style={[s.closeBtnText, { color: theme.textSecondary }]}>
              Done
            </Text>
          </TouchableOpacity>
        </View>

        {/* Tabs */}
        <ScrollView
          horizontal
          showsHorizontalScrollIndicator={false}
          contentContainerStyle={s.tabs}
        >
          {(
            [
              { key: "presets", label: "Presets" },
              { key: "recordings", label: "Recordings" },
              { key: "display", label: "Display" },
              { key: "about", label: "About" },
              ...(__DEV__ ? [{ key: "logs" as const, label: "Logs" }] : []),
            ] as { key: "presets" | "recordings" | "display" | "about" | "logs"; label: string }[]
          ).map((tab) => (
            <TouchableOpacity
              key={tab.key}
              style={[
                s.tab,
                activeTab === tab.key && {
                  borderBottomColor: theme.primary,
                  borderBottomWidth: 2,
                },
              ]}
              onPress={() => setActiveTab(tab.key)}
            >
              <Text
                style={[
                  s.tabText,
                  {
                    color:
                      activeTab === tab.key
                        ? theme.primary
                        : theme.textSecondary,
                  },
                ]}
              >
                {tab.label}
              </Text>
            </TouchableOpacity>
          ))}
        </ScrollView>
      </SafeAreaView>

      {/* Content */}
      <ScrollView
        style={s.scroll}
        contentContainerStyle={s.scrollContent}
        showsVerticalScrollIndicator={false}
      >
        {/* ── Presets ── */}
        {activeTab === "presets" && (
          <View style={s.section}>
            {fmPresets.length > 0 && (
              <>
                <Text style={[s.sectionLabel, { color: theme.textSecondary }]}>
                  FM STATIONS
                </Text>
                {fmPresets.map((p) => (
                  <PresetRow
                    key={p.id}
                    preset={p}
                    onRename={(id, name) => updatePreset(id, { name })}
                    onDelete={deletePreset}
                    theme={theme}
                  />
                ))}
              </>
            )}

            {amPresets.length > 0 && (
              <>
                <Text
                  style={[
                    s.sectionLabel,
                    { color: theme.textSecondary, marginTop: 16 },
                  ]}
                >
                  AM STATIONS
                </Text>
                {amPresets.map((p) => (
                  <PresetRow
                    key={p.id}
                    preset={p}
                    onRename={(id, name) => updatePreset(id, { name })}
                    onDelete={deletePreset}
                    theme={theme}
                  />
                ))}
              </>
            )}

            {fmPresets.length === 0 && amPresets.length === 0 && (
              <View style={s.emptyState}>
                <Text style={[s.emptyIcon, { color: theme.textDim }]}>☆</Text>
                <Text style={[s.emptyTitle, { color: theme.textSecondary }]}>
                  No saved presets
                </Text>
                <Text style={[s.emptyBody, { color: theme.textDim }]}>
                  Tap the star icon on the radio screen to bookmark a station.
                </Text>
              </View>
            )}
          </View>
        )}

        {/* ── Recordings ── */}
        {activeTab === "recordings" && (
          <View style={s.section}>
            <Text style={[s.sectionLabel, { color: theme.textSecondary }]}>
              SAVED RECORDINGS
            </Text>

            <View
              style={[
                s.card,
                { backgroundColor: theme.surface, borderColor: theme.border },
              ]}
            >
              <Text style={[s.cardTitle, { color: theme.text }]}>
                Recordings Folder
              </Text>
              <Text style={[s.cardBody, { color: theme.textSecondary }]}>
                /SDRGo/Recordings
              </Text>
              <TouchableOpacity
                style={[
                  s.cardBtn,
                  {
                    backgroundColor: theme.surfaceRaised,
                    borderColor: theme.border,
                  },
                ]}
                onPress={handleOpenRecordingsFolder}
              >
                <Text style={[s.cardBtnText, { color: theme.primary }]}>
                  Open in Files ↗
                </Text>
              </TouchableOpacity>
            </View>

            <View style={s.emptyState}>
              <Text style={[s.emptyIcon, { color: theme.textDim }]}>⏺</Text>
              <Text style={[s.emptyTitle, { color: theme.textSecondary }]}>
                No recordings yet
              </Text>
              <Text style={[s.emptyBody, { color: theme.textDim }]}>
                Use the record button on the radio screen to capture audio.
                {"\n"}Recordings are named by timestamp and station.
              </Text>
            </View>
          </View>
        )}

        {/* ── Display ── */}
        {activeTab === "display" && (
          <View style={s.section}>
            <Text style={[s.sectionLabel, { color: theme.textSecondary }]}>
              WAVEFORM STYLE
            </Text>
            {(
              [
                {
                  key: "artistic",
                  label: "Artistic Aurora",
                  desc: "Ambient glow that breathes with the audio signal",
                },
                {
                  key: "oscilloscope",
                  label: "Oscilloscope",
                  desc: "Raw waveform trace, phosphor CRT style",
                },
              ] as const
            ).map((opt) => (
              <TouchableOpacity
                key={opt.key}
                style={[
                  s.optionRow,
                  {
                    backgroundColor:
                      visualMode === opt.key
                        ? theme.primaryGlow
                        : theme.surface,
                    borderColor:
                      visualMode === opt.key ? theme.primary : theme.border,
                  },
                ]}
                onPress={() => onVisualModeChange(opt.key)}
              >
                <View style={s.optionText}>
                  <Text style={[s.optionLabel, { color: theme.text }]}>
                    {opt.label}
                  </Text>
                  <Text style={[s.optionDesc, { color: theme.textSecondary }]}>
                    {opt.desc}
                  </Text>
                </View>
                <View
                  style={[
                    s.optionRadio,
                    {
                      borderColor:
                        visualMode === opt.key ? theme.primary : theme.border,
                      backgroundColor:
                        visualMode === opt.key ? theme.primary : "transparent",
                    },
                  ]}
                />
              </TouchableOpacity>
            ))}

            <Text
              style={[
                s.sectionLabel,
                { color: theme.textSecondary, marginTop: 24 },
              ]}
            >
              THEME
            </Text>
            {(
              [
                {
                  key: "system",
                  label: "System Default",
                  desc: "Follows device dark / light setting",
                },
                {
                  key: "dark",
                  label: "Dark",
                  desc: "Deep OLED instrument panel",
                },
                {
                  key: "light",
                  label: "Light",
                  desc: "Warm parchment, easy on the eyes",
                },
              ] as const
            ).map((opt) => (
              <TouchableOpacity
                key={opt.key}
                style={[
                  s.optionRow,
                  {
                    backgroundColor:
                      themeMode === opt.key ? theme.primaryGlow : theme.surface,
                    borderColor:
                      themeMode === opt.key ? theme.primary : theme.border,
                  },
                ]}
                onPress={() => setThemeMode(opt.key)}
              >
                <View style={s.optionText}>
                  <Text style={[s.optionLabel, { color: theme.text }]}>
                    {opt.label}
                  </Text>
                  <Text style={[s.optionDesc, { color: theme.textSecondary }]}>
                    {opt.desc}
                  </Text>
                </View>
                <View
                  style={[
                    s.optionRadio,
                    {
                      borderColor:
                        themeMode === opt.key ? theme.primary : theme.border,
                      backgroundColor:
                        themeMode === opt.key ? theme.primary : "transparent",
                    },
                  ]}
                />
              </TouchableOpacity>
            ))}
          </View>
        )}

        {/* ── Logs (dev only) ── */}
        {activeTab === "logs" && __DEV__ && (
          <View style={s.section}>
            <View style={s.logsHeader}>
              <Text style={[s.sectionLabel, { color: theme.textSecondary }]}>
                DEVICE LOG ({logEntries.length})
              </Text>
              <TouchableOpacity onPress={clearLogEntries}>
                <Text style={[s.logsClearBtn, { color: theme.danger }]}>
                  Clear
                </Text>
              </TouchableOpacity>
            </View>

            {logEntries.length === 0 ? (
              <View style={s.emptyState}>
                <Text style={[s.emptyTitle, { color: theme.textSecondary }]}>
                  No log entries yet
                </Text>
                <Text style={[s.emptyBody, { color: theme.textDim }]}>
                  Trigger an action on the radio screen to see output here.
                </Text>
              </View>
            ) : (
              [...logEntries].reverse().map((entry) => (
                <View
                  key={entry.id}
                  style={[s.logRow, { borderBottomColor: theme.border }]}
                >
                  <Text style={[s.logTime, { color: theme.textDim }]}>
                    {entry.time}
                  </Text>
                  <Text
                    style={[
                      s.logLevel,
                      {
                        color:
                          entry.level === "error"
                            ? theme.danger
                            : entry.level === "warn"
                              ? theme.signal
                              : theme.textDim,
                      },
                    ]}
                  >
                    {entry.level.toUpperCase()}
                  </Text>
                  <Text
                    style={[s.logMessage, { color: theme.text }]}
                    selectable
                  >
                    {entry.message}
                  </Text>
                </View>
              ))
            )}
          </View>
        )}

        {/* ── About ── */}
        {activeTab === "about" && (
          <View style={s.section}>
            <View
              style={[
                s.card,
                { backgroundColor: theme.surface, borderColor: theme.border },
              ]}
            >
              <Text style={[s.appName, { color: theme.primary }]}>
                SDRGo FM
              </Text>
              <Text style={[s.appVersion, { color: theme.textSecondary }]}>
                v0.1.0 — RTL-SDR Android Radio
              </Text>
            </View>

            {[
              {
                title: "Signal Processing",
                body: "Rust sdr_core — FM demodulation, stereo pilot detection, 5-band EQ",
              },
              {
                title: "Hardware",
                body: "RTL-SDR Blog V3 via USB OTG. 0.5 – 1700 MHz coverage.",
              },
              {
                title: "RDS Decoder",
                body: "In development. Station name, radio text, PTY and AF will decode automatically when complete.",
              },
              {
                title: "AM Demodulator",
                body: "In development. Controls, dial, and settings are fully functional.",
              },
              {
                title: "Recording",
                body: "Kotlin AudioTrack capture. Records to /SDRGo/Recordings as WAV.",
              },
            ].map((item) => (
              <View
                key={item.title}
                style={[s.aboutRow, { borderBottomColor: theme.border }]}
              >
                <Text style={[s.aboutTitle, { color: theme.text }]}>
                  {item.title}
                </Text>
                <Text style={[s.aboutBody, { color: theme.textSecondary }]}>
                  {item.body}
                </Text>
              </View>
            ))}
          </View>
        )}
      </ScrollView>
    </View>
  );
}

function styles(theme: any, insets: any) {
  return StyleSheet.create({
    root: { flex: 1 },
    headerWrap: {
      borderBottomWidth: StyleSheet.hairlineWidth,
      borderBottomColor: theme.border,
    },
    header: {
      flexDirection: "row",
      alignItems: "center",
      justifyContent: "space-between",
      paddingHorizontal: 20,
      paddingTop: 16,
      paddingBottom: 12,
    },
    title: { fontSize: 20, fontWeight: "700" },
    closeBtn: { paddingHorizontal: 4 },
    closeBtnText: { fontSize: 17 },
    tabs: {
      flexDirection: "row",
      paddingHorizontal: 16,
      gap: 0,
    },
    tab: {
      paddingHorizontal: 16,
      paddingVertical: 12,
      borderBottomWidth: 2,
      borderBottomColor: "transparent",
    },
    tabText: {
      fontSize: 14,
      fontWeight: "600",
    },
    scroll: { flex: 1 },
    scrollContent: { padding: 20, gap: 12, paddingBottom: insets.bottom + 32 },
    section: { gap: 10 },
    sectionLabel: {
      fontSize: 10,
      fontWeight: "800",
      letterSpacing: 2.5,
      marginBottom: 2,
    },
    card: {
      borderRadius: 14,
      borderWidth: StyleSheet.hairlineWidth,
      padding: 16,
      gap: 6,
    },
    cardTitle: { fontSize: 15, fontWeight: "700" },
    cardBody: { fontSize: 12, fontFamily: "monospace" },
    cardBtn: {
      alignSelf: "flex-start",
      marginTop: 6,
      paddingHorizontal: 14,
      paddingVertical: 8,
      borderRadius: 8,
      borderWidth: StyleSheet.hairlineWidth,
    },
    cardBtnText: { fontSize: 13, fontWeight: "700" },
    optionRow: {
      flexDirection: "row",
      alignItems: "center",
      padding: 14,
      borderRadius: 12,
      borderWidth: 1,
      gap: 12,
    },
    optionText: { flex: 1, gap: 3 },
    optionLabel: { fontSize: 14, fontWeight: "600" },
    optionDesc: { fontSize: 12 },
    optionRadio: {
      width: 20,
      height: 20,
      borderRadius: 10,
      borderWidth: 2,
    },
    emptyState: {
      alignItems: "center",
      paddingVertical: 48,
      gap: 8,
    },
    emptyIcon: { fontSize: 40 },
    emptyTitle: { fontSize: 16, fontWeight: "600" },
    emptyBody: {
      fontSize: 13,
      textAlign: "center",
      lineHeight: 20,
      paddingHorizontal: 24,
    },
    aboutRow: {
      borderBottomWidth: StyleSheet.hairlineWidth,
      paddingVertical: 14,
      gap: 4,
    },
    aboutTitle: { fontSize: 14, fontWeight: "700" },
    aboutBody: { fontSize: 13, lineHeight: 18 },
    appName: { fontSize: 28, fontWeight: "800", letterSpacing: 3 },
    appVersion: { fontSize: 12, fontFamily: "monospace" },
    logsHeader: {
      flexDirection: "row",
      alignItems: "center",
      justifyContent: "space-between",
    },
    logsClearBtn: { fontSize: 13, fontWeight: "700" },
    logRow: {
      flexDirection: "row",
      alignItems: "flex-start",
      gap: 8,
      paddingVertical: 6,
      borderBottomWidth: StyleSheet.hairlineWidth,
    },
    logTime: {
      fontSize: 11,
      fontFamily: "monospace",
      paddingTop: 1,
      width: 58,
    },
    logLevel: {
      fontSize: 10,
      fontWeight: "800",
      fontFamily: "monospace",
      paddingTop: 2,
      width: 36,
    },
    logMessage: {
      flex: 1,
      fontSize: 12,
      fontFamily: "monospace",
      lineHeight: 17,
    },
  });
}

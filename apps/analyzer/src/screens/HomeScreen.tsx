import React, { useEffect, useState } from "react";
import {
  View,
  Text,
  StyleSheet,
  TouchableOpacity,
  ScrollView,
  Dimensions,
  StatusBar,
} from "react-native";
import { useTheme, SdrModule } from "@sdrgo/ui-core";

const { width } = Dimensions.get("window");
const CARD_WIDTH = (width - 48) / 2;

interface ModeCard {
  id: string;
  label: string;
  subtitle: string;
  icon: string;
  route: string;
  frequencyRange: string;
}

const MODES: ModeCard[] = [
  {
    id: "fm",
    label: "FM / AM",
    subtitle: "Music & Radio",
    icon: "📻",
    route: "FM",
    frequencyRange: "87.5 – 108 MHz",
  },
  {
    id: "airband",
    label: "Air Band",
    subtitle: "ATC Chatter",
    icon: "✈️",
    route: "AirBand",
    frequencyRange: "118 – 137 MHz",
  },
  {
    id: "noaa",
    label: "NOAA",
    subtitle: "Weather & Satellite",
    icon: "🛰️",
    route: "NOAA",
    frequencyRange: "162.4 – 162.55 MHz",
  },
  {
    id: "adsb",
    label: "ADS-B",
    subtitle: "Live Air Traffic",
    icon: "🗺️",
    route: "ADSB",
    frequencyRange: "1090 MHz",
  },
  {
    id: "analyzer",
    label: "Analyzer",
    subtitle: "Spectrum & Waterfall",
    icon: "📊",
    route: "Analyzer",
    frequencyRange: "0.5 – 1700 MHz",
  },
  {
    id: "settings",
    label: "Settings",
    subtitle: "Device & Audio",
    icon: "⚙️",
    route: "Settings",
    frequencyRange: "",
  },
];

export default function HomeScreen({ navigation }: any) {
  const { theme, isDark } = useTheme();
  const [coreVersion, setCoreVersion] = useState("");
  const [deviceConnected, setDeviceConnected] = useState(false);

  useEffect(() => {
    SdrModule.getCoreVersion()
      .then(setCoreVersion)
      .catch(() => {});
  }, []);

  const s = styles(theme, isDark);

  return (
    <View style={s.container}>
      <StatusBar
        barStyle={isDark ? "light-content" : "dark-content"}
        backgroundColor={theme.background}
      />

      {/* Header */}
      <View style={s.header}>
        <View>
          <Text style={s.appName}>SDRGo</Text>
          <Text style={s.version}>{coreVersion}</Text>
        </View>
        <View
          style={[
            s.deviceIndicator,
            {
              backgroundColor: deviceConnected ? theme.success : theme.textDim,
            },
          ]}
        >
          <Text style={s.deviceIndicatorText}>
            {deviceConnected ? "● LIVE" : "○ NO DEVICE"}
          </Text>
        </View>
      </View>

      {/* Mode cards */}
      <ScrollView
        contentContainerStyle={s.grid}
        showsVerticalScrollIndicator={false}
      >
        {MODES.map((mode) => (
          <TouchableOpacity
            key={mode.id}
            style={s.card}
            onPress={() => navigation.navigate(mode.route)}
            activeOpacity={0.7}
          >
            <Text style={s.cardIcon}>{mode.icon}</Text>
            <Text style={s.cardLabel}>{mode.label}</Text>
            <Text style={s.cardSubtitle}>{mode.subtitle}</Text>
            {mode.frequencyRange ? (
              <Text style={s.cardFreq}>{mode.frequencyRange}</Text>
            ) : null}
            <View style={[s.cardAccent, { backgroundColor: theme.primary }]} />
          </TouchableOpacity>
        ))}
      </ScrollView>
    </View>
  );
}

const styles = (theme: any, isDark: boolean) =>
  StyleSheet.create({
    container: {
      flex: 1,
      backgroundColor: theme.background,
    },
    header: {
      flexDirection: "row",
      justifyContent: "space-between",
      alignItems: "center",
      paddingHorizontal: 20,
      paddingTop: 60,
      paddingBottom: 24,
    },
    appName: {
      fontSize: 32,
      fontWeight: "800",
      color: theme.primary,
      letterSpacing: 3,
    },
    version: {
      fontSize: 10,
      color: theme.textSecondary,
      fontFamily: "monospace",
      marginTop: 2,
    },
    deviceIndicator: {
      paddingHorizontal: 12,
      paddingVertical: 6,
      borderRadius: 20,
    },
    deviceIndicatorText: {
      fontSize: 11,
      fontWeight: "700",
      color: "#000",
      letterSpacing: 1,
    },
    grid: {
      flexDirection: "row",
      flexWrap: "wrap",
      paddingHorizontal: 16,
      gap: 12,
      paddingBottom: 32,
    },
    card: {
      width: CARD_WIDTH,
      backgroundColor: theme.surface,
      borderRadius: 16,
      padding: 20,
      borderWidth: 1,
      borderColor: theme.border,
      overflow: "hidden",
      minHeight: 140,
      justifyContent: "flex-end",
    },
    cardIcon: {
      fontSize: 28,
      marginBottom: 8,
    },
    cardLabel: {
      fontSize: 18,
      fontWeight: "700",
      color: theme.text,
      letterSpacing: 0.5,
    },
    cardSubtitle: {
      fontSize: 12,
      color: theme.textSecondary,
      marginTop: 2,
    },
    cardFreq: {
      fontSize: 10,
      color: theme.textDim,
      fontFamily: "monospace",
      marginTop: 6,
    },
    cardAccent: {
      position: "absolute",
      top: 0,
      right: 0,
      width: 4,
      height: "100%",
      borderTopRightRadius: 16,
      borderBottomRightRadius: 16,
      opacity: 0.6,
    },
  });

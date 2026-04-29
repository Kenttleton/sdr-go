import React from "react";
import { NavigationContainer } from "@react-navigation/native";
import { createNativeStackNavigator } from "@react-navigation/native-stack";
import { SafeAreaProvider } from "react-native-safe-area-context";

// From shared package now
import { ThemeProvider, useTheme } from "@sdrgo/ui-core";

// Screens live locally in this app
import HomeScreen from "./src/screens/HomeScreen";
import FmScreen from "./src/screens/FmScreen";
import AirBandScreen from "./src/screens/AirBandScreen";
import NoaaScreen from "./src/screens/NoaaScreen";
import AdsbScreen from "./src/screens/AdsbScreen";
import AnalyzerScreen from "./src/screens/AnalyzerScreen";
import SettingsScreen from "./src/screens/SettingsScreen";

const Stack = createNativeStackNavigator();

function AppNavigator() {
  const { theme, isDark } = useTheme();

  return (
    <NavigationContainer>
      <Stack.Navigator
        screenOptions={{
          headerStyle: { backgroundColor: theme.surface },
          headerTintColor: theme.primary,
          headerTitleStyle: {
            fontWeight: "700",
          },
          headerBackTitle: "",
          contentStyle: { backgroundColor: theme.background },
        }}
      >
        <Stack.Screen
          name="Home"
          component={HomeScreen}
          options={{ headerShown: false }}
        />
        <Stack.Screen
          name="FM"
          component={FmScreen}
          options={{ title: "FM / AM Tuner" }}
        />
        <Stack.Screen
          name="AirBand"
          component={AirBandScreen}
          options={{ title: "Air Band" }}
        />
        <Stack.Screen
          name="NOAA"
          component={NoaaScreen}
          options={{ title: "NOAA Weather" }}
        />
        <Stack.Screen
          name="ADSB"
          component={AdsbScreen}
          options={{ title: "ADS-B Traffic" }}
        />
        <Stack.Screen
          name="Analyzer"
          component={AnalyzerScreen}
          options={{ title: "Spectrum Analyzer" }}
        />
        <Stack.Screen
          name="Settings"
          component={SettingsScreen}
          options={{ title: "Settings" }}
        />
      </Stack.Navigator>
    </NavigationContainer>
  );
}

export default function App() {
  return (
    <SafeAreaProvider>
      <ThemeProvider>
        <AppNavigator />
      </ThemeProvider>
    </SafeAreaProvider>
  );
}

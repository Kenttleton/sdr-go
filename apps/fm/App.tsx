import React from "react";
import { SafeAreaProvider } from "react-native-safe-area-context";
import { ThemeProvider } from "@sdrgo/ui-core";
import RadioScreen from "./src/screens/RadioScreen";

export default function App() {
  return (
    <SafeAreaProvider>
      <ThemeProvider>
        <RadioScreen />
      </ThemeProvider>
    </SafeAreaProvider>
  );
}

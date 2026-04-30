import { registerRootComponent } from 'expo';
import { View, Text, StyleSheet } from 'react-native';
import { driverError } from '@sdrgo/ui-core';
import App from './App';

function DriverErrorScreen() {
    return (
        <View style={styles.container}>
            <Text style={styles.title}>SDRGo FM</Text>
            <Text style={styles.error}>{driverError}</Text>
        </View>
    );
}

const styles = StyleSheet.create({
    container: {
        flex: 1,
        backgroundColor: '#08070a',
        alignItems: 'center',
        justifyContent: 'center',
        padding: 24,
    },
    title: {
        fontSize: 36,
        fontWeight: '800',
        color: '#f5a623',
        letterSpacing: 4,
        marginBottom: 16,
    },
    error: {
        fontSize: 13,
        color: '#ff3b5c',
        fontFamily: 'monospace',
        textAlign: 'center',
    },
});

registerRootComponent(driverError ? DriverErrorScreen : App);
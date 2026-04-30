// SDRGo Instrument Panel Theme
// Aesthetic: Deep OLED cockpit dark, warm amber accents, phosphor green signal
// Light variant: warm parchment with dark ink, same amber accent family

export const colors = {
  dark: {
    // Backgrounds — true OLED blacks with warm undertone
    background:     '#08070a',
    surface:        '#111018',
    surfaceRaised:  '#1a1826',
    surfaceMid:     '#13111e',
    overlay:        '#0d0b14ee',

    // Borders
    border:         '#252235',
    borderSubtle:   '#1a1828',

    // Accent — warm amber, instrument readout
    primary:        '#f5a623',   // warm amber
    primaryDim:     '#f5a62322',
    primaryGlow:    '#f5a62344',
    primaryBright:  '#ffc04a',

    // Signal green — phosphor CRT
    signal:         '#39ff8f',   // phosphor green
    signalDim:      '#39ff8f22',

    // Danger / recording
    danger:         '#ff3b5c',
    dangerDim:      '#ff3b5c22',

    // Text
    text:           '#f0ece8',
    textSecondary:  '#8a8098',
    textDim:        '#3d3850',
    textInverse:    '#08070a',

    // Waveform / spectrum palette
    waterfall0:     '#08070a',
    waterfall1:     '#0f0c1a',
    waterfall2:     '#1a0d33',
    waterfall3:     '#330d66',
    waterfall4:     '#5c0f99',
    waterfall5:     '#9900cc',
    waterfall6:     '#cc33ff',
    waterfall7:     '#ff99ee',
    waterfall8:     '#ffcc77',
    waterfall9:     '#ffffff',

    // Stereo / mono indicator
    stereo:         '#39ff8f',
    mono:           '#f5a623',
  },

  light: {
    background:     '#f7f4ef',
    surface:        '#ffffff',
    surfaceRaised:  '#ede9e1',
    surfaceMid:     '#f3efe8',
    overlay:        '#f7f4efee',

    border:         '#d4cfc6',
    borderSubtle:   '#e8e4dc',

    primary:        '#c97d10',
    primaryDim:     '#c97d1022',
    primaryGlow:    '#c97d1044',
    primaryBright:  '#e8921a',

    signal:         '#1a8f4f',
    signalDim:      '#1a8f4f22',

    danger:         '#d42040',
    dangerDim:      '#d4204022',

    text:           '#1a1520',
    textSecondary:  '#7a7080',
    textDim:        '#c0bcc8',
    textInverse:    '#f7f4ef',

    waterfall0:     '#f7f4ef',
    waterfall1:     '#e8e2d0',
    waterfall2:     '#d4c8a0',
    waterfall3:     '#c0a060',
    waterfall4:     '#b07820',
    waterfall5:     '#c97d10',
    waterfall6:     '#e8921a',
    waterfall7:     '#ff8c00',
    waterfall8:     '#ff4400',
    waterfall9:     '#cc0000',

    stereo:         '#1a8f4f',
    mono:           '#c97d10',
  },
};

export type Theme = typeof colors.dark;
export type ThemeMode = 'dark' | 'light' | 'system';
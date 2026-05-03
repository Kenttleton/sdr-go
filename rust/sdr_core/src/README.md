# SDR Core

SDR Core is the low-level library that handles all of the signal processing and hardware interaction. The Kotlin layer acts as a passthrough to the React Native UI that users will interact with in a domain based app.

## Features

Spectrum Analysis

- Waterfall
- Waveform
- FFT

Analog Demodulation

- FM
  - Wide (WFM)
  - Narrow (NFM)
- AM
  - Upper Side Band (USB)
  - Lower Side Band (LSB)
  - Double Side Band (DSB)
- Continuous Wave (CW)
- RAW I/Q

De-emphasis Options

- None
- US, Canada, South Korea, and Japan - 75 \(\mu s\)
- Europe - 50 \(\mu s\)
- Custom

Digital Demodulation

- Frequency Shift Keying (FSK)
- Phase Shift Keying (PSK)
- Quadrature Amplitude Modulation (QAM)
- Orthogonal Frequency-Division Multiplexing (OFDM)
- Amplitude Shift Keying (ASK)

Filters

- 7 Band EQ
- Noise Reduction
- Noise Blanking
- Notch filters
- Squelch
- Pre-gain - hardware based gain
- Post-gain - software based gain

## Pipelines

Pipelines should be dynamic and can be implemented in real-time to process signals into an audio, visual, or data output the android app can do something with. For example; to tune into an FM radio station:

FM Processing Chain

```text
RTL-SDR USB bytes
        ↓
[1] Byte → IQ conversion (bias removal, normalize)
        ↓
[2] Complex FIR LPF at 56kHz, 128 taps   ← runs at 2,048,000 Hz
        ↓
[3] Decimate by 8  →  256,000 Hz IQ stream
        ↓
[4] FM Polar Discriminator  →  256,000 Hz real baseband
        ↓
    ┌───────────────────────────────────────┐
    │          SPLIT into 3 paths           │
    │                                       │
[5a] LPF 15kHz (L+R)    [5b] BPF 19kHz   [5c] BPF 57kHz
    │                         (Pilot)          (RDS)
    │                           ↓               ↓
    │                   [6] Stereo PLL    [7] RDS BPSK
    │                   get 38kHz ref         demod
    │                           ↓               ↓
[5d] BPF 23-53kHz        [8] Pilot        [9] Clock
    (L-R subcarrier)          level           recovery
    ↓ × ref_38kHz             detect           ↓
    LPF 15kHz                   ↓         [10] CRC + frame
    (L-R recovered)        is_stereo?          decode
    │                           │               ↓
    └──────────┬────────────────┘           RdsData
               ↓
[11] Decimate by 5  →  ~51,200 Hz
               ↓
[12] De-emphasis (75µs)  ← MUST be at audio rate
               ↓
[13] AGC (normalize level)
               ↓
[14] 7-band EQ (biquad cascade)
               ↓
[15] Soft clip / limiter (protect ears and speakers)
               ↓
[16] Write to Android AudioTrack (48000 Hz stereo float)
```

To tune into an AM radio station:

AM Processing Chain

```text
RTL-SDR (or direct sampling for HF)
        ↓
[1] IQ conversion (same as FM)
        ↓
[2] Pre-filter LPF — ±5kHz cutoff
    (much narrower than FM → fewer taps needed, less CPU)
        ↓
[3] Decimate aggressively → 51,200 Hz
    (AM audio bandwidth only needs ~10kHz)
        ↓
[4] Choose demodulation mode:
    ├── Envelope detect (AM, fast path)
    ├── Synchronous PLL (SAM, better quality)
    └── BFO offset PLL (USB/LSB for SSB)
        ↓
[5] Noise blanker
    (impulse noise removal — essential for AM)
        ↓
[6] IF filter (variable bandwidth)
    ├── Wide: 8kHz    (music, strong signals)
    ├── Normal: 5kHz  (standard broadcast)
    ├── Narrow: 3kHz  (interference, adjacent channel)
    └── Voice: 2.5kHz (severe interference, SSB)
        ↓
[7] Auto-notch filter
    (heterodyne whistle removal)
        ↓
[8] AGC with hang time
    (must be after noise blanker to avoid gain pumping on blanked noise)
        ↓
[9] De-emphasis
    (AM does NOT have pre-emphasis — this is different from FM)
    (Optional high-frequency lift to compensate for IF filter rolloff)
        ↓
[10] 7-band EQ
     (same biquad cascade as FM)
     (typical AM EQ: boost 1-3kHz presence, cut below 100Hz rumble)
        ↓
[11] Audio output (mono — AM is always mono)
```

Other pipelines are still TBD.

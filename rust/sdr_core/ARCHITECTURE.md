# sdr_core — Architecture

Internal developer reference. Covers module layout, signal processing chains, the pipeline state machine, and planned additions.

---

## Module layout

```
src/
  lib.rs
  usb/
    device.rs
    stream.rs
    hardware.rs
  pipeline/
    mod.rs
    manager.rs
    fm.rs
    am.rs
    ddc.rs
    filters.rs
    spectrum.rs
```

### `usb/`

**`device.rs` — `SdrDevice`**
Wraps `rtl-sdr-rs` for the Blog V3. Owns the USB file descriptor, handles device open/close, center frequency changes, and gain control. `available_gains()` returns the device's supported gain table in tenths of dB. Bulk transfer size is negotiated at open time.

**`stream.rs` — `IqStream`**
Ring buffer that absorbs the uneven USB bulk transfer cadence and presents a clean `drain(n)` interface to the pipeline. After a hardware retune, `mark_retuned()` starts a settling countdown (204 800 samples ≈ 100 ms at 2.048 MSPS) during which `fill()` reads are discarded. This covers the R820T2 PLL lock time and AGC re-settling.

**`hardware.rs`**
RTL2832U / R820T2 register-level configuration — sample rate, tuner init, direct sampling mode for HF.

---

### `pipeline/`

**`mod.rs` — `DemodPipeline`**
An enum (`Fm(FmPipeline)` | `Am(AmPipeline)`) that provides a common `process_iq`, `freeze_agc`, and `is_stereo_detected` interface. Mode-specific controls (`set_fm_stereo`, `set_am_mode`, `set_am_bandwidth`) are dispatched here.

**`manager.rs` — `PipelineManager`**
Top-level orchestrator. Owns the active `DemodPipeline`, a `Ddc`, and the current `PipelineState`. JNI calls (`setFrequency`, `setMode`) drive state transitions; `getAudioBuffer` advances the machine each call via `process_iq`.

**`ddc.rs` — `Ddc`**
Digital down-converter. Multiplies each IQ sample by a complex exponential to shift the center frequency in software. Used for tuning offsets within ±1 MHz of the hardware center — no USB round-trip, no PLL settling, no glitch.

**`filters.rs`**
Kaiser-windowed FIR design (`firdes::lowpass`), `FirFilter` (direct-form convolution), and `DecimatingFirFilter` / `ComplexDecimatingFirFilter` (combined filter + integer decimation). No external DSP library dependency — all filters are computed from scratch.

**`spectrum.rs` — `FftStage` + `WaveformStage`**
`FftStage` runs a Hann-windowed FFT (rustfft) on raw IQ and returns magnitude bins in dBFS. The FFT reads directly from the IQ ring buffer in `lib.rs`, independent of the demod state.

`WaveformStage` captures two 512-sample display snapshots opportunistically from data already in flight: the IQ envelope (pre-demod magnitude) and the audio PCM (post-demod). Both snapshots are updated in `getAudioBuffer` and polled by the frontend via `getIqWaveform` / `getAudioWaveform`. Neither touches the main audio path.

---

## Pipeline state machine

`PipelineManager` uses a `PipelineState` enum to coordinate transitions cleanly:

```
Stable
  │
  ├─ setFrequency (offset > ±1 MHz) ──► Retuning { samples_remaining }
  │     hardware retune + mark_retuned()       └─ drain empty until zero ──► Stable
  │
  ├─ setFrequency (offset ≤ ±1 MHz) ──► DDC offset updated in Stable (no state change)
  │
  └─ setMode ──────────────────────────► CrossfadingMode { outgoing, progress, crossfade_len }
        new DemodPipeline built               └─ blend outgoing/incoming PCM over 2048 samples
        outgoing = old pipeline                    incoming AGC frozen during blend
        incoming = new pipeline (AGC frozen)   └─ when progress >= len ──► Stable
```

During `Retuning`, `process_iq` returns an empty `Vec` so Kotlin's audio loop gets silence while the PLL settles. During `CrossfadingMode`, both pipelines run on every IQ block and their outputs are linearly blended (`alpha = progress / crossfade_len`).

---

## FM demodulation chain (WFM — current)

```
IQ samples (2.048 MSPS)
        │
[DDC]   shift center frequency digitally if offset ≤ ±1 MHz
        │
[1] Polar discriminator
        inst_freq = atan2(im(s × conj(prev)), re(s × conj(prev)))
        → real baseband at input_rate
        │
[2] Pre-filter + decimate (DecimatingFirFilter)
        LPF cutoff 100 kHz, Kaiser β=50
        decimation = input_rate / 200_000
        → ~200 kHz intermediate rate
        │
[3] Pilot tracking (19 kHz NCO)
        correlation with pilot_phase.cos() → pilot_amplitude (slow IIR)
        stereo_detected = pilot_amplitude > 0.05
        produces 38 kHz reference: (pilot_phase × 2).cos()
        │
       ┌──────────────────────┐
[4]  sum LPF (FirFilter)    [5] diff LPF (FirFilter)
     cutoff 15 kHz               cutoff 15 kHz
     (L+R mono signal)           applied to disc × 38 kHz ref → (L−R)
       └──────────────────────┘
        │
[6] Decimate to output rate (step_by diff_decimation)
    de-emphasis IIR (75 µs, US/Canada/Japan standard)
    AGC (slow: gain down 0.1 %/sample when amp > 0.5, up 0.01 %/sample otherwise)
        │
[7] Interleave → [L0, R0, L1, R1, …]
```

Stereo is automatic: when `stereo_detected && stereo_enabled`, L = (sum+diff)/2, R = (sum−diff)/2. When the pilot is absent, L = R = sum (mono fallback). There is no manual mono mode — the signal determines the output.

---

## FM demodulation chain (NFM — planned)

NFM uses the same polar discriminator and pipeline structure as WFM with narrower filter parameters:

- Pre-filter cutoff: 12.5 kHz (vs. 100 kHz for WFM)
- No pilot tracking — NFM is always mono
- No stereo path
- De-emphasis: none by default (voice communications)
- Expected deviation: ±5 kHz (narrowband voice)

Implementation: add `NfmPipeline` or parameterize `FmPipeline` with a mode flag. Expose as `PipelineMode::Nfm`.

---

## AM demodulation chain (current)

```
IQ samples
        │
[1] Pre-filter + decimate (ComplexDecimatingFirFilter)
        LPF cutoff 6 kHz, Kaiser β=50
        decimation = input_rate / 50_000
        → ~51 kHz intermediate rate
        │
[2] Envelope detect
        demod = |s| = (re² + im²).sqrt()
        (SAM / USB / LSB are stubbed — all fall through to envelope for now)
        │
[3] DC block (IIR, ~30 Hz cutoff)
        removes carrier residual
        │
[4] IF filter (FirFilter, variable bandwidth)
        Wide 8 kHz / Normal 5 kHz / Narrow 3 kHz / Voice 2.5 kHz
        │
[5] AGC + decimate to output rate
        step_by(intermediate / output)
        gain down 0.1 %/sample when |s| > 0.8, up 0.01 % otherwise
        │
[6] Mono → interleaved stereo [L0, R0, L1, R1, …] (L == R)
```

SAM, USB, and LSB share the `AmMode` enum but all currently run envelope detection. The infrastructure (mode switching, bandwidth control) is in place; the demod math for each is the next step.

---

## Parametric EQ (planned)

Up to 7 bands, each a biquad section. Each band has three parameters and a filter type:

| Parameter | Range | Description |
|---|---|---|
| `freq` | 20 – 20 000 Hz | Center (bell) or corner (shelf) frequency |
| `gain_db` | −24 to +24 dB | Boost or cut at center/corner |
| `q` | 0.1 – 10.0 | Bandwidth — higher Q = narrower bell |
| `kind` | 0 / 1 / 2 | 0 = Bell, 1 = Low Shelf, 2 = High Shelf |

Biquad coefficients are computed from these parameters using the Audio EQ Cookbook (RBJ) formulas. The EQ runs at audio rate, after de-emphasis and AGC, before the samples are returned to Kotlin.

**JNI interface:** `setEq(bands: FloatArray)` where `bands` is packed as `[freq0, gain0, q0, kind0, freq1, …]` (4 floats per band, `kind` cast to `f32`). Empty array disables the EQ. `count = bands.size / 4`.

**Placement in chain:**
```
… AGC → [EQ biquad cascade] → interleave → getAudioBuffer()
```

**Why Rust:** biquad processing runs on every audio sample (48 000–96 000/s). Kotlin-side processing would cross the JNI boundary on every buffer, adding allocation pressure and JNI overhead on the hot path. Rust keeps it in the same `getAudioBuffer` call.

---

## RDS decoder (planned, WFM only)

RDS uses a 57 kHz subcarrier (= 19 kHz pilot × 3) modulated with BPSK at 1187.5 bps. The 200 kHz intermediate rate in the FM pipeline is sufficient to capture it.

### Signal chain

```
After polar discriminator, before pre-filter decimation:

[A] 57 kHz bandpass filter (BPF, ~2 kHz wide)
        extract subcarrier from baseband
        │
[B] Multiply by 57 kHz reference (derived from pilot PLL × 3)
        BPSK demodulation → bipolar symbol stream
        │
[C] Matched filter (root raised-cosine, symbol rate 1187.5 Hz)
        │
[D] Clock recovery (Gardner timing error detector)
        → symbol decisions at 1187.5 samples/s
        │
[E] Differential decoding (DBPSK → NRZ bits)
        │
[F] Bit synchronizer → 26-bit groups (16 data + 10 CRC)
        │
[G] CRC-10 syndrome check (generator polynomial 0x5B9)
        correct single-bit errors; drop groups that fail
        │
[H] Group type decode
        0A/0B → PS (8 chars)
        2A/2B → RadioText (64 chars)
        header → PI (16-bit station ID), PTY (5-bit genre), TP, TA, MS
```

### Output

`getRdsInfo()` returns a JSON string. Empty object `{}` when no RDS data received.

```json
{
  "pi":  "ABCD",
  "ps":  "CALL FM ",
  "rt":  "Now playing: Artist - Track Title",
  "pty": 4,
  "tp":  true,
  "ta":  false,
  "ms":  true
}
```

---

## Signal strength (planned)

IQ power (RMS) measured as a parallel output, similar to the waveform snapshots. No filtering of the audio path — measured directly from the raw IQ block in `getAudioBuffer` before it enters the demod.

```rust
// In WaveformStage:
pub fn update_rms(&mut self, iq: &[Cf32]) {
    let sum: f32 = iq.iter().map(|s| s.re * s.re + s.im * s.im).sum();
    self.signal_strength = (sum / iq.len() as f32).sqrt();
}

pub fn signal_strength(&self) -> f32 {
    self.signal_strength
}
```

Exposed via `getSignalStrength() -> jfloat`. Returns a value in `[0.0, 1.0]` — normalized IQ amplitude, not dBm. The Kotlin scan engine uses this to determine whether a station is present at a given frequency during a band scan.

---

## Frequency transition strategy

Two strategies exist depending on offset magnitude. Both are active:

### DDC (software tune, offset ≤ ±1 MHz)

`Ddc` multiplies each sample by `exp(j·2π·offset·n/Fs)`. Instantaneous — no gap, no settling, no state change in `PipelineManager`. `setFrequency` returns 1.

### Hardware retune (offset > ±1 MHz)

`SdrDevice::set_frequency` sends the new frequency to the R820T2. The PLL needs ~1 ms to lock; AGC re-settles over ~100 ms. `IqStream::mark_retuned()` sets a 204 800-sample countdown. During `Retuning`, `process_iq` returns an empty vec — Kotlin's audio loop gets silence. DDC offset is cleared after a hardware retune since hardware is now at the target frequency. `setFrequency` returns 2.

### Remaining: AdaptiveFirFilter (planned)

Abrupt filter coefficient swaps (e.g. WFM → NFM bandwidth change) produce a click from the impulse response transient. The fix is a crossfade between old and new filter over ~512 samples. Not yet implemented — bandwidth changes currently use an immediate coefficient swap.

---

## JNI design decisions

**Single global `PIPELINE`:** One `Lazy<Mutex<Option<Pipeline>>>` owns all state. The Kotlin audio loop is the only hot caller; all other JNI calls are control-path and infrequent. `parking_lot::Mutex` is used over `std::sync::Mutex` to avoid priority inversion on Android.

**`getAudioBuffer` drives everything:** The Kotlin producer coroutine calls `getAudioBuffer` in a tight loop. This single call does USB fill, IQ drain, waveform snapshot update, and demodulation. All other JNI calls (spectrum, waveform, signal strength) read side-channel state populated by this loop.

**Empty array as "no data":** All data-returning functions return an empty array rather than null when nothing is available. Kotlin's JNI binding handles `jfloatArray` as a non-nullable type; the empty array is the correct sentinel.

**JNI naming:** All exported symbols follow `Java_com_sdrgo_SdrModule_<methodName>`. The package and class names are baked into the symbol — changing either requires rebuilding the `.so`.

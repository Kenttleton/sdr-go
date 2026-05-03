# SDR Smooth Transitions: Mode, Bandwidth & Frequency

## Context
RTL-SDR streams continuous IQ at fixed sample rate (2.048 MSPS). Switching center
frequency, bandwidth, or demodulation mode mid-stream causes glitches without care.
This doc covers the patterns to implement in `rust/sdr_core/`.

---

## 1. Center Frequency Transitions

### Hardware retune — discard settling samples
After `set_center_freq()`, the R820T2 PLL needs ~1ms to lock and AGC ~50-100ms to
re-settle. Discard ~200K samples (100ms @ 2MSPS) post-retune.

Add to `usb/stream.rs` → `IqStream`:
```rust
settling_samples: usize,  // countdown after retune

pub fn mark_retuned(&mut self) {
    self.settling_samples = 204_800; // 100ms @ 2.048MSPS
}

// In fill(): if settling_samples > 0, zero-fill or advance read_pos past bad data
```

### Digital retuning (DDC) — glitch-free small offsets
RTL-SDR has ~2.4MHz usable BW. For offsets within ±1MHz, never touch hardware —
multiply IQ by a complex exponential instead. Instantaneous, no settling needed.

Add to `dsp/` as `ddc.rs`:
```rust
pub fn digital_retune(samples: &mut [Complex<f32>], offset_hz: f32, sample_rate: f32) {
    let phase_inc = 2.0 * PI * offset_hz / sample_rate;
    let mut phase = 0.0f32;
    for s in samples.iter_mut() {
        *s *= Complex::new(phase.cos(), phase.sin());
        phase = (phase + phase_inc) % (2.0 * PI);
    }
}
```

`FmDemodulator` should hold a DDC offset rather than always sitting at DC.
Hardware retune only when offset would exceed ±1MHz.

---

## 2. Bandwidth Changes (Filter Switching)

Abrupt coefficient swaps create a click (impulse response transient).
Fix: crossfade between old and new filter over ~512 samples.

Replace `FirFilter` usage in `dsp/filters.rs` with an `AdaptiveFirFilter`:
```rust
pub struct AdaptiveFirFilter {
    current: FirFilter,
    target: Option<FirFilter>,
    crossfade_pos: usize,
    crossfade_len: usize,  // 512 samples recommended
}

// transition_to() sets target + resets counter
// process() linearly interpolates: a*(1-α) + b*α until crossfade_len reached,
// then swaps current ← target
```

Used in `FmDemodulator` for `pre_filter`, `audio_filter_sum`, `audio_filter_diff`
when switching WFM↔NFM (200kHz→12.5kHz cutoff change).

---

## 3. Demodulation Mode Switching (FM ↔ AM ↔ SSB)

### Output crossfade (implement first)
Run both old and new demodulators on the same IQ block during transition,
linearly mix their outputs over ~1024 samples. Freeze AGC during crossfade.

```rust
// In Pipeline or a CrossfadingDemod wrapper:
// 1. switch_mode() saves pending mode, resets crossfade counter
// 2. process() runs both demodulators, mixes with alpha = pos/len
// 3. When pos >= len, drop old demodulator, clear pending
```

### Shared pre-processing pipeline (structural goal)
All modes should share the same front-end chain:
```
IQ → DDC shift → Channelizer/LPF → [FM disc | AM envelope | SSB mixer] → AGC → audio
```
Switching demodulators doesn't disturb the front-end. Only the final math changes.

### AGC freeze during transitions
Add `frozen: bool` to `Agc` in `dsp/agc.rs`. Freeze on `switch_mode()`,
unfreeze after crossfade completes. Prevents gain surge from amplitude
characteristic mismatch between modes.

---

## 4. Pipeline State Machine

Replace imperative commands in `src/lib.rs` `Pipeline` with a state machine.
JNI calls (`setFrequency`, future `setMode`, `setBandwidth`) trigger state
transitions; `getAudioBuffer` advances the machine each call.

```rust
enum PipelineState {
    Stable,
    Retuning { samples_remaining: usize },
    CrossfadingMode {
        outgoing: Box<dyn Demodulator>,
        incoming: Box<dyn Demodulator>,
        progress: f32,
    },
    CrossfadingBandwidth { filter: AdaptiveFirFilter },
}
```

A `Demodulator` trait unifies FM/AM/SSB behind a common interface:
```rust
pub trait Demodulator: Send {
    fn process(&mut self, iq: &[Complex<f32>]) -> Vec<f32>;
    fn mode(&self) -> DemodMode;
}
```

---

## 5. Implementation Order

1. `IqStream::mark_retuned()` + settling discard — fixes current retune noise
2. `AdaptiveFirFilter` — fixes bandwidth click
3. `Ddc` struct in `dsp/ddc.rs` — enables glitch-free small frequency steps
4. `Demodulator` trait — prerequisite for crossfading
5. `CrossfadingDemod` wrapper — smooth mode switches
6. `PipelineState` machine in `lib.rs` — ties everything together

---

## Key Numbers (RTL-SDR @ 2.048 MSPS)

| Event | Discard / Crossfade Length |
|---|---|
| Hardware retune settling | ~204,800 samples (100ms) |
| Filter crossfade | 512 samples (~0.25ms) |
| Mode crossfade | 1,024 samples (~0.5ms) |
| DDC usable range before hardware retune | ±1 MHz |
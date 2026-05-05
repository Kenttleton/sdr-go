# Network / Service Layer

Thin wrapper around `PipelineManager` that exposes radio control and audio streaming over WebSocket. Does not touch DSP.

## Architecture

```rust
struct RadioService {
    pipeline: PipelineManager,
}
```

## Control API — JSON over WebSocket

Commands sent from client to server:

```json
{ "cmd": "tune",   "freq": 101100000 }
{ "cmd": "mode",   "value": "wfm" }
{ "cmd": "stereo", "enabled": true }
```

## Audio Streaming

**Phase 1** — raw PCM over WebSocket binary frames (fastest to ship)

**Phase 2** — Opus encoding (better bandwidth)

Relevant crates: `opus`, `audiopus`

## Server Loop

```rust
loop {
    let iq = read_samples();
    let pcm = pipeline.process_iq(iq);

    send_audio(pcm);
    send_metadata(rssi, stereo_detected);
}
```

## Metadata Channel

Sent periodically alongside audio:

```json
{
  "rssi": -42.3,
  "stereo": true,
  "mode": "wfm"
}
```

## Deployment model

JNI and the network service are mutually exclusive entry points — you run one or the other, not both simultaneously.

- **Android**: `lib.rs` JNI exports + the `PIPELINE` global, called from Kotlin.
- **Network service**: a separate binary crate that depends on `sdr_core` and calls `service::RadioService` directly. The JNI glue is not compiled in.

`PipelineManager` is the single source of truth for both paths.

## Lock-free control

`RadioService` drains a `std::sync::mpsc` channel at the top of each IQ block via `try_recv` — non-blocking, no mutex in the hot path. The WebSocket handler holds a `RadioServiceHandle` (cheaply cloneable `Sender<Command>`) and fires commands without ever touching the DSP state directly.

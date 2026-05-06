# sdr_srv

HTTP/WebSocket network service that exposes `sdr_core` over a REST API. Useful for desktop or embedded Linux deployments where you want to control an RTL-SDR from a remote client.

## Quick start

```sh
# From the workspace root
cargo run -p sdr_srv
```

The server binds to `0.0.0.0:8080`. Open the interactive API docs in a browser:

```
http://localhost:8080/docs
```

The Swagger UI at `/docs` lets you call every REST endpoint directly and shows full request/response schemas.

The machine-readable OpenAPI spec is at:

```
http://localhost:8080/openapi.json
```

## Hardware

By default the server runs on a stub IQ source (silence) so it starts without a physical device attached. It will log a warning:

```
WARN no RTL-SDR devices found — DSP thread will run on stub IQ
```

To connect a real RTL-SDR, edit `src/main.rs` and replace the stub loop with a real `SdrDevice`/`IqStream` pair. Available `DeviceSource` options are documented in the `TODO` comment in `main.rs`.

## REST endpoints

All paths are under `/api`. Request and response bodies are JSON.

| Method | Path | Description |
|---|---|---|
| `GET` | `/api/metadata` | Current RSSI, stereo lock state, and center frequency |
| `POST` | `/api/tune` | Set receive frequency (`freq_hz: u32`) |
| `POST` | `/api/mode` | Set demodulation mode (`mode`: `wfm` \| `nfm` \| `am_dsb` \| `am_usb` \| `am_lsb`) |
| `POST` | `/api/stereo` | Enable/disable FM stereo decoding (`enabled: bool`) |
| `POST` | `/api/squelch` | Set squelch threshold (`threshold_db: f32`, `hang_ms: f32`) |

### Tuning behaviour

Small frequency changes (≤ 1 MHz from the current center) use DDC software tuning and take effect immediately. Larger jumps issue a hardware retune command; the caller is responsible for re-opening or re-configuring the physical device and calling `/api/tune` again once the oscillator has settled.

## WebSocket

Connect to `ws://localhost:8080/ws/audio` for a multiplexed audio + metadata stream.

- **Binary frames** — raw PCM audio as little-endian `f32` samples, interleaved (stereo when stereo is enabled).
- **Text frames** — JSON metadata snapshot sent every 500 ms:
  ```json
  { "rssi_db": -42.1, "stereo_detected": true, "center_hz": 101100000 }
  ```

## Environment

| Variable | Default | Description |
|---|---|---|
| `RUST_LOG` | _(none)_ | Log filter, e.g. `RUST_LOG=info` or `RUST_LOG=sdr_srv=debug` |

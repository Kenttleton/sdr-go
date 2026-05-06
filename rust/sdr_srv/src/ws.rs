use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{ConnectInfo, State, WebSocketUpgrade};
use axum::response::Response;
use serde::Serialize;
use std::net::SocketAddr;

use crate::AppState;

#[derive(Serialize)]
struct MetadataEvent {
    rssi_db: f32,
    stereo_detected: bool,
    center_hz: u32,
}

pub async fn handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<AppState>>,
) -> Response {
    tracing::info!(%addr, "WS client connecting");
    ws.on_upgrade(move |socket| stream(socket, state, addr))
}

async fn stream(mut socket: WebSocket, state: Arc<AppState>, addr: SocketAddr) {
    tracing::info!(%addr, "WS session started");
    let mut audio_rx = state.audio_tx.subscribe();
    let mut meta_tick = tokio::time::interval(Duration::from_millis(500));
    let mut audio_frames: u64 = 0;

    loop {
        tokio::select! {
            result = audio_rx.recv() => {
                match result {
                    Ok(pcm) => {
                        audio_frames += 1;
                        if audio_frames % 200 == 0 {
                            tracing::debug!(%addr, audio_frames, "WS audio heartbeat");
                        }
                        let bytes: Vec<u8> = pcm.iter()
                            .flat_map(|s: &f32| s.to_le_bytes())
                            .collect();
                        if socket.send(Message::Binary(bytes.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::warn!(%addr, error = %e, "WS audio broadcast lagged or closed");
                        break;
                    }
                }
            }
            _ = meta_tick.tick() => {
                let event = {
                    let m = state.metadata.lock().unwrap();
                    MetadataEvent {
                        rssi_db: m.rssi_db,
                        stereo_detected: m.stereo_detected,
                        center_hz: m.center_hz,
                    }
                };
                tracing::debug!(%addr, rssi_db = event.rssi_db, stereo = event.stereo_detected, "WS meta tick");
                let json = serde_json::to_string(&event).unwrap();
                if socket.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) if text.trim() == "disconnect" => {
                        tracing::info!(%addr, "WS client requested disconnect");
                        break;
                    }
                    None | Some(Ok(Message::Close(_))) => {
                        tracing::info!(%addr, "WS client closed connection");
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    tracing::info!(%addr, audio_frames, "WS session closed");
}

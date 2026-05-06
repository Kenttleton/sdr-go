use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode};
use sdr_core::pipeline::PipelineMode;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::AppState;

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub struct MetadataResponse {
    pub rssi_db: f32,
    pub stereo_detected: bool,
    pub center_hz: u32,
}

// ── Request types ─────────────────────────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub struct TuneRequest {
    /// Target receive frequency in Hz.
    pub freq_hz: u32,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModeValue {
    Wfm,
    Nfm,
    AmDsb,
    AmUsb,
    AmLsb,
}

#[derive(Deserialize, ToSchema)]
pub struct ModeRequest {
    pub mode: ModeValue,
}

#[derive(Deserialize, ToSchema)]
pub struct StereoRequest {
    pub enabled: bool,
}

#[derive(Deserialize, ToSchema)]
pub struct SquelchRequest {
    pub threshold_db: f32,
    pub hang_ms: f32,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/metadata",
    responses((status = 200, body = MetadataResponse)),
    tag = "radio"
)]
pub async fn get_metadata(
    State(state): State<Arc<AppState>>,
) -> Json<MetadataResponse> {
    let m = state.metadata.lock().unwrap();
    tracing::debug!(rssi_db = m.rssi_db, center_hz = m.center_hz, "GET /api/metadata");
    Json(MetadataResponse {
        rssi_db: m.rssi_db,
        stereo_detected: m.stereo_detected,
        center_hz: m.center_hz,
    })
}

#[utoipa::path(
    post,
    path = "/api/tune",
    request_body = TuneRequest,
    responses((status = 200, description = "Tuned"), (status = 500, description = "Pipeline disconnected")),
    tag = "radio"
)]
pub async fn post_tune(
    State(state): State<Arc<AppState>>,
    Json(body): Json<TuneRequest>,
) -> StatusCode {
    let ok = state.handle.set_channel_frequency(body.freq_hz).is_ok();
    tracing::info!(freq_hz = body.freq_hz, ok, "POST /api/tune");
    if ok { StatusCode::OK } else { StatusCode::INTERNAL_SERVER_ERROR }
}

#[utoipa::path(
    post,
    path = "/api/mode",
    request_body = ModeRequest,
    responses((status = 200, description = "Mode set"), (status = 500, description = "Pipeline disconnected")),
    tag = "radio"
)]
pub async fn post_mode(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ModeRequest>,
) -> StatusCode {
    let mode = match body.mode {
        ModeValue::Wfm   => PipelineMode::Wfm,
        ModeValue::Nfm   => PipelineMode::Nfm,
        ModeValue::AmDsb => PipelineMode::AmDsb,
        ModeValue::AmUsb => PipelineMode::AmUsb,
        ModeValue::AmLsb => PipelineMode::AmLsb,
    };
    let ok = state.handle.set_mode(mode).is_ok();
    tracing::info!(mode = ?body.mode, ok, "POST /api/mode");
    if ok { StatusCode::OK } else { StatusCode::INTERNAL_SERVER_ERROR }
}

#[utoipa::path(
    post,
    path = "/api/stereo",
    request_body = StereoRequest,
    responses((status = 200, description = "Stereo set"), (status = 500, description = "Pipeline disconnected")),
    tag = "radio"
)]
pub async fn post_stereo(
    State(state): State<Arc<AppState>>,
    Json(body): Json<StereoRequest>,
) -> StatusCode {
    let ok = state.handle.set_fm_stereo(body.enabled).is_ok();
    tracing::info!(enabled = body.enabled, ok, "POST /api/stereo");
    if ok { StatusCode::OK } else { StatusCode::INTERNAL_SERVER_ERROR }
}

#[utoipa::path(
    post,
    path = "/api/squelch",
    request_body = SquelchRequest,
    responses((status = 200, description = "Squelch set"), (status = 500, description = "Pipeline disconnected")),
    tag = "radio"
)]
pub async fn post_squelch(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SquelchRequest>,
) -> StatusCode {
    let ok = state.handle.set_squelch(body.threshold_db, body.hang_ms).is_ok();
    tracing::info!(threshold_db = body.threshold_db, hang_ms = body.hang_ms, ok, "POST /api/squelch");
    if ok { StatusCode::OK } else { StatusCode::INTERNAL_SERVER_ERROR }
}

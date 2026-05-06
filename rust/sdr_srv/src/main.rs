mod routes;
mod settings;
mod ws;

use std::sync::{Arc, Mutex};

use axum::{Router, routing::{get, post}};
use std::net::SocketAddr;
use sdr_core::service::{Metadata, RadioService, RadioServiceHandle};
use sdr_core::usb::{DeviceConfig, DeviceSource, SdrDevice};
use settings::{AppConfig, DeviceSettings};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::Level;
use tracing_subscriber::EnvFilter;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub struct AppState {
    pub handle: RadioServiceHandle,
    pub metadata: Arc<Mutex<Metadata>>,
    pub audio_tx: broadcast::Sender<Vec<f32>>,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        routes::get_metadata,
        routes::post_tune,
        routes::post_mode,
        routes::post_stereo,
        routes::post_squelch,
    ),
    components(schemas(
        routes::MetadataResponse,
        routes::TuneRequest,
        routes::ModeRequest,
        routes::ModeValue,
        routes::StereoRequest,
        routes::SquelchRequest,
    )),
    tags((name = "radio", description = "SDR radio control"))
)]
struct ApiDoc;

fn init_logging(cfg: &settings::LoggingSettings) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&cfg.level));
    match cfg.format.as_str() {
        "json" => tracing_subscriber::fmt().with_env_filter(filter).json().init(),
        "compact" => tracing_subscriber::fmt().with_env_filter(filter).compact().init(),
        _ => tracing_subscriber::fmt().with_env_filter(filter).init(),
    }
}

fn resolve_device_source(cfg: &DeviceSettings) -> Option<DeviceSource> {
    match cfg.source.as_str() {
        "stub" | "" => None,
        "first_available" => Some(DeviceSource::FirstAvailable),
        "index" => Some(DeviceSource::Index(cfg.index.unwrap_or(0))),
        "serial" => cfg.serial.clone().map(DeviceSource::Serial),
        "vid_pid" => {
            let vid = u16::from_str_radix(cfg.vid.as_deref().unwrap_or(""), 16)
                .ok()?;
            let pid = u16::from_str_radix(cfg.pid.as_deref().unwrap_or(""), 16)
                .ok()?;
            Some(DeviceSource::VidPid { vid, pid })
        }
        other => {
            eprintln!("unknown device.source '{}', falling back to stub", other);
            None
        }
    }
}

#[tokio::main]
async fn main() {
    let cfg = AppConfig::load().expect("failed to load config");
    init_logging(&cfg.logging);

    tracing::info!(
        source = %cfg.device.source,
        center_hz = cfg.device.center_hz,
        sample_rate = cfg.device.sample_rate,
        audio_rate = cfg.device.audio_rate,
        "device settings"
    );

    let (audio_tx, _) = broadcast::channel(16);
    let metadata = Arc::new(Mutex::new(Metadata {
        rssi_db: -100.0,
        stereo_detected: false,
        center_hz: cfg.device.center_hz,
    }));

    let devices = SdrDevice::enumerate();
    if devices.is_empty() {
        tracing::warn!("no RTL-SDR devices found — DSP thread will run on stub IQ");
    } else {
        for d in &devices {
            tracing::info!(
                "[{}] {:04x}:{:04x}  {}  {}  serial={}",
                d.index, d.vendor_id, d.product_id, d.manufacturer, d.product, d.serial
            );
        }
    }

    let stream = resolve_device_source(&cfg.device).and_then(|src| {
        let dev_cfg = DeviceConfig {
            frequency_hz: cfg.device.center_hz,
            sample_rate: cfg.device.sample_rate,
            gain_tenths: cfg.device.gain_tenths,
            bias_tee: cfg.device.bias_tee,
            audio_sample_rate: cfg.device.audio_rate,
        };
        match SdrDevice::open(src, dev_cfg) {
            Ok(dev) => {
                tracing::info!("RTL-SDR opened — wiring IqStream");
                Some(dev.into_stream())
            }
            Err(e) => {
                tracing::warn!("failed to open RTL-SDR: {} — using stub IQ", e);
                None
            }
        }
    });

    let (mut service, handle) = RadioService::new(
        cfg.device.sample_rate,
        cfg.device.audio_rate,
        true,
        cfg.device.center_hz,
        stream,
    );

    let meta_writer = Arc::clone(&metadata);
    let audio_writer = audio_tx.clone();

    std::thread::spawn(move || loop {
        let pcm = service.tick().to_vec();
        let _ = audio_writer.send(pcm);
        *meta_writer.lock().unwrap() = service.metadata();
    });

    let state = Arc::new(AppState { handle, metadata, audio_tx });

    let app = Router::new()
        .merge(SwaggerUi::new("/docs").url("/openapi.json", ApiDoc::openapi()))
        .route("/api/metadata", get(routes::get_metadata))
        .route("/api/tune", post(routes::post_tune))
        .route("/api/mode", post(routes::post_mode))
        .route("/api/stereo", post(routes::post_stereo))
        .route("/api/squelch", post(routes::post_squelch))
        .route("/ws/audio", get(ws::handler))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        );

    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await
        .unwrap_or_else(|e| panic!("failed to bind {}: {}", addr, e));
    tracing::info!("listening on {}  —  docs at http://localhost:{}/docs", addr, cfg.server.port);
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap();
}

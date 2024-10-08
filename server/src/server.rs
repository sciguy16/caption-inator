use crate::{ControlMessage, Line, Result, RunState};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
    routing::{get, post},
    Json, Router,
};
use std::{path::PathBuf, time::Duration};
use tokio::sync::{broadcast, mpsc, oneshot};
use tower_http::services::ServeDir;
use tracing::info;

const LISTEN_ADDRESS: &str = "[::1]:3000";
const PING_INTERVAL: Duration = Duration::from_secs(5);
const GET_STATUS_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
struct AppState {
    tx: broadcast::Sender<Line>,
    control_tx: mpsc::Sender<ControlMessage>,
}

pub async fn run(
    tx: broadcast::Sender<Line>,
    control_tx: mpsc::Sender<ControlMessage>,
    frontend: Option<PathBuf>,
) -> Result<()> {
    let mut app = Router::new()
        .route("/api/", get(|| async { "Hello, World!" }))
        .route("/api/subscribe", get(ws_subscribe))
        .route("/api/azure/start", post(start))
        .route("/api/azure/stop", post(stop))
        .route("/api/azure/simulate", post(simulate))
        .route("/api/azure/status", get(status))
        .with_state(AppState { tx, control_tx });

    if let Some(frontend) = frontend {
        let serve_dir = ServeDir::new(frontend);
        app = app.fallback_service(serve_dir);
    }

    info!("Server listening on http://{LISTEN_ADDRESS}");
    let listener = tokio::net::TcpListener::bind(LISTEN_ADDRESS).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn ws_subscribe(
    State(AppState { tx, .. }): State<AppState>,
    ws: WebSocketUpgrade,
) -> Response {
    debug!("New websocket connection");
    let rx = tx.subscribe();
    ws.on_upgrade(|ws| async move {
        if let Err(err) = handle_websocket(ws, rx).await {
            warn!("Websocket closed: `{err}`");
        }
    })
}

async fn handle_websocket(
    mut socket: WebSocket,
    mut rx: broadcast::Receiver<Line>,
) -> Result<()> {
    let mut ping_interval = tokio::time::interval(PING_INTERVAL);

    loop {
        tokio::select! {
            _ = ping_interval.tick() => {
                socket.send(Message::Ping(vec![0])).await?;
            }
            Some(Ok(msg)) = socket.recv() => {
                if let Message::Close(_) = msg { break }
            }
            line = rx.recv() => {
                socket.send(Message::Text(serde_json::to_string(&line?)?))
                    .await?;
            }
        }
    }

    Ok(())
}

async fn start(State(AppState { control_tx, .. }): State<AppState>) {
    info!("Start");
    control_tx
        .send(ControlMessage::SetState(RunState::Running))
        .await
        .unwrap();
}

async fn stop(State(AppState { control_tx, .. }): State<AppState>) {
    info!("Stop");
    control_tx
        .send(ControlMessage::SetState(RunState::Stopped))
        .await
        .unwrap();
}

async fn simulate(State(AppState { control_tx, .. }): State<AppState>) {
    info!("Simulation");
    control_tx
        .send(ControlMessage::SetState(RunState::Test))
        .await
        .unwrap();
}

async fn status(
    State(AppState { control_tx, .. }): State<AppState>,
) -> Json<RunState> {
    info!("Status");
    let (tx, rx) = oneshot::channel();
    control_tx.send(ControlMessage::GetState(tx)).await.unwrap();
    Json(
        tokio::time::timeout(GET_STATUS_TIMEOUT, rx)
            .await
            .unwrap()
            .unwrap(),
    )
}

use crate::{Line, Result, RunState};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
    routing::{get, post},
    Json, Router,
};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::{broadcast, Mutex};
use tower_http::services::ServeDir;
use tracing::info;

const LISTEN_ADDRESS: &str = "[::1]:3000";
const PING_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Clone)]
struct AppState {
    tx: broadcast::Sender<Line>,
    run_state: Arc<Mutex<RunState>>,
}

pub async fn run(
    tx: broadcast::Sender<Line>,
    frontend: Option<PathBuf>,
) -> Result<()> {
    let mut app = Router::new()
        .route("/api/", get(|| async { "Hello, World!" }))
        .route("/api/subscribe", get(ws_subscribe))
        .route("/api/azure/start", post(start))
        .route("/api/azure/stop", post(stop))
        .route("/api/azure/simulate", post(simulate))
        .route("/api/azure/status", get(status))
        .with_state(AppState {
            tx,
            run_state: Arc::new(Mutex::new(RunState::Stopped)),
        });

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

async fn start(State(AppState { run_state, .. }): State<AppState>) {
    info!("Start");
    let mut state = run_state.lock().await;
    *state = RunState::Running;
}

async fn stop(State(AppState { run_state, .. }): State<AppState>) {
    info!("Stop");
    let mut state = run_state.lock().await;
    *state = RunState::Stopped;
}

async fn simulate(State(AppState { run_state, .. }): State<AppState>) {
    info!("Simulation");
    let mut state = run_state.lock().await;
    *state = RunState::Test;
}

async fn status(
    State(AppState { run_state, .. }): State<AppState>,
) -> Json<RunState> {
    info!("Status");
    let state = run_state.lock().await;
    Json(*state)
}

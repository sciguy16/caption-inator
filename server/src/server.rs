use crate::{ControlMessage, Language, Line, Result, RunState};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
    routing::{get, post},
    Json, Router,
};
use std::{net::SocketAddr, path::PathBuf, time::Duration};
use tokio::sync::{broadcast, mpsc, oneshot};
use tower_http::services::ServeDir;
use tracing::info;

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
    listen_address: SocketAddr,
) -> Result<()> {
    let mut app = Router::new()
        .route("/api/", get(|| async { "Hello, World!" }))
        .route("/api/subscribe", get(ws_subscribe))
        .route("/api/azure/start", post(start))
        .route("/api/azure/stop", post(stop))
        .route("/api/azure/simulate", post(simulate))
        .route("/api/azure/status", get(status))
        .route("/api/ip", get(ip))
        .route("/api/lang", get(get_lang).post(post_lang))
        .with_state(AppState { tx, control_tx });

    if let Some(frontend) = frontend {
        let serve_dir = ServeDir::new(frontend);
        app = app.fallback_service(serve_dir);
    }

    info!("Server listening on http://{listen_address}");
    let listener = tokio::net::TcpListener::bind(listen_address).await?;
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

async fn ip() -> Json<String> {
    use std::process::Stdio;
    info!("IP");

    let routes = std::process::Command::new("ip")
        .arg("route")
        .stdout(Stdio::piped())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&routes.stdout);
    for line in stdout.lines() {
        if line.starts_with("default") {
            let mut iterator = line.split_whitespace();
            while let Some(element) = iterator.next() {
                if element == "dev" {
                    let default_if = iterator.next().unwrap();
                    info!(?default_if);

                    let addresses = std::process::Command::new("ip")
                        .args(["addr", "show", "dev", default_if])
                        .stdout(Stdio::piped())
                        .output()
                        .unwrap();
                    let addresses = String::from_utf8_lossy(&addresses.stdout);
                    let addresses = addresses
                        .lines()
                        .filter(|line| line.contains("global"))
                        .filter_map(|line| line.split_whitespace().nth(1))
                        .collect::<Vec<_>>();
                    return Json(addresses.join(", "));
                }
            }
        }
    }
    warn!("No interface found");

    Json("Failed to find interface".into())
}

async fn get_lang(
    State(AppState { control_tx, .. }): State<AppState>,
) -> Json<Language> {
    info!("Get lang");
    let (tx, rx) = oneshot::channel();
    control_tx
        .send(ControlMessage::GetLanguage(tx))
        .await
        .unwrap();
    Json(
        tokio::time::timeout(GET_STATUS_TIMEOUT, rx)
            .await
            .unwrap()
            .unwrap(),
    )
}

async fn post_lang(
    app_state: State<AppState>,
    Json(req): Json<String>,
) -> Json<Language> {
    info!("Set language: {req}");
    app_state
        .control_tx
        .send(ControlMessage::SetLanguage(req))
        .await
        .unwrap();
    get_lang(app_state).await
}

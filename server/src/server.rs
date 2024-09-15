use crate::{Line, Result};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
    routing::get,
    Router,
};
use std::time::Duration;
use tokio::sync::broadcast;

const LISTEN_ADDRESS: &str = "[::1]:3000";
const PING_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Clone)]
struct AppState {
    tx: broadcast::Sender<Line>,
}

pub async fn run(tx: broadcast::Sender<Line>) -> Result<()> {
    let app = Router::new()
        .route("/api/", get(|| async { "Hello, World!" }))
        .route("/api/subscribe", get(ws_subscribe))
        .with_state(AppState { tx });

    info!("Server listening on http://{LISTEN_ADDRESS}");
    let listener = tokio::net::TcpListener::bind(LISTEN_ADDRESS).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn ws_subscribe(
    State(AppState { tx }): State<AppState>,
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

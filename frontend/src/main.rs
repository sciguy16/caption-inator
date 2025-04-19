use futures_util::StreamExt;
use gloo::net::websocket::{futures::WebSocket, Message};
use serde::Deserialize;
use std::{collections::VecDeque, rc::Rc, time::Duration};
use wasm_bindgen::UnwrapThrowExt;
use yew::prelude::*;

mod controls;

const WEBSOCKET_URL: &str = "/api/subscribe";
const CAPTION_BUFFER_LEN: usize = 5;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
enum ConnectionState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
}

impl ConnectionState {
    pub fn is_disconnected(&self) -> bool {
        *self == Self::Disconnected
    }
}

#[derive(Clone)]
struct CaptionBuffer {
    sentences: VecDeque<Rc<str>>,
    active: Option<Rc<str>>,
}

impl Default for CaptionBuffer {
    fn default() -> Self {
        Self {
            sentences: VecDeque::with_capacity(CAPTION_BUFFER_LEN),
            active: None,
        }
    }
}

impl CaptionBuffer {
    fn push(&mut self, line: Line) {
        match line {
            Line::Recognising(line) => {
                self.active = Some(line.into());
            }
            Line::Recognised(line) => {
                if self.sentences.len() >= CAPTION_BUFFER_LEN {
                    let _ = self.sentences.pop_front();
                }
                self.sentences.push_back(line.into());
                self.active = None;
            }
        }
    }
}

#[derive(Deserialize)]
enum Line {
    Recognising(String),
    Recognised(String),
}

#[function_component]
fn App() -> Html {
    html! {
            <Captions />
    }
}

#[function_component]
fn Captions() -> Html {
    let window = web_sys::window().unwrap_throw();
    let connection_state = use_state_eq(ConnectionState::default);
    let buffer = use_state(CaptionBuffer::default);

    window.scroll_by_with_x_and_y(0.0, 2000.0);

    if connection_state.is_disconnected() {
        connection_state.set(ConnectionState::Connecting);
        wasm_bindgen_futures::spawn_local({
            gloo::console::log!("connect to websocket");
            let mut ws = WebSocket::open(WEBSOCKET_URL).unwrap_throw();
            let connection_state = connection_state.clone();
            let buffer = buffer.clone();

            async move {
                connection_state.set(ConnectionState::Connected);

                // The Rc() value store in the UseStateHandle gets stale,
                // so instead of re-cloning it on every message we retain
                // the active state here and push it out to the component
                // whenever it gets updated
                let mut new = (*buffer).clone();

                while let Some(Ok(msg)) = ws.next().await {
                    // gloo::console::log!(format!("message: {msg:?}"));
                    if let Message::Text(msg) = msg {
                        match serde_json::from_str(&msg) {
                            Ok(msg) => {
                                new.push(msg);
                                buffer.set(new.clone());
                            }
                            Err(err) => {
                                gloo::console::error!(err.to_string());
                                break;
                            }
                        }
                    }
                }
                yew::platform::time::sleep(Duration::from_secs(1)).await;
                connection_state.set(ConnectionState::Disconnected);
                gloo::console::log!("Websocket closed");
            }
        });
    }

    let sentences = buffer
        .sentences
        .iter()
        .map(|sentence| html! { <p class="sentence">{ sentence }</p>})
        .collect::<Html>();
    let active = buffer.active.clone().map_or_else(
        || html!(),
        |active| html! { <p class="active">{ active }</p> },
    );

    html! {
        <>
            <p id="header">
                <span class="state">
                    { format!("State: {:?}; ", *connection_state) }
                </span>
                <controls::Controls />
            </p>
            <div class="container">
                { sentences }
                { active }
            </div>
        </>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}

use gloo::net::http::Request;
use serde::Deserialize;
use yew::prelude::*;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Deserialize)]
enum RunState {
    #[default]
    Unknown,
    Stopped,
    Running,
    Test,
}

#[function_component]
pub fn Controls() -> Html {
    let run_state = use_state_eq(RunState::default);
    let ip = use_state_eq(String::default);

    let onsubmit = |evt: SubmitEvent| {
        evt.prevent_default();
    };

    // check the current run state
    wasm_bindgen_futures::spawn_local({
        let run_state = run_state.clone();
        let ip = ip.clone();

        async move {
            let new_state = Request::get("/api/azure/status")
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            run_state.set(new_state);

            let new_ip = Request::get("/api/ip")
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            ip.set(new_ip);
        }
    });

    let start = {
        let run_state = run_state.clone();
        move |_| {
            let run_state = run_state.clone();
            wasm_bindgen_futures::spawn_local(async move {
                Request::post("/api/azure/start").send().await.unwrap();
                run_state.set(RunState::default());
            });
        }
    };

    let stop = {
        let run_state = run_state.clone();
        move |_| {
            let run_state = run_state.clone();
            wasm_bindgen_futures::spawn_local(async move {
                Request::post("/api/azure/stop").send().await.unwrap();
                run_state.set(RunState::default());
            });
        }
    };

    let simulate = {
        let run_state = run_state.clone();
        move |_| {
            let run_state = run_state.clone();
            wasm_bindgen_futures::spawn_local(async move {
                Request::post("/api/azure/simulate").send().await.unwrap();
                run_state.set(RunState::default());
            });
        }
    };

    html! {
        <form {onsubmit}>
            { format!("Captions {:?}; IP: {}", *run_state, *ip) }
            <p>
                <button onclick={start}>{ "Start" }</button>
                <button onclick={stop}>{ "Stop" }</button>
                <button onclick={simulate}>{ "Test" }</button>
                { " Language: " }
                <select>
                    <option>{ "English" }</option>
                    <option>{ "English (IE)" }</option>
                    <option>{ "English (US)" }</option>
                    <option>{ "Japanese" }</option>
                </select>
            </p>
        </form>
    }
}

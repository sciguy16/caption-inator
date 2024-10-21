use gloo::net::http::Request;
use serde::Deserialize;
use wasm_bindgen::JsCast;
use web_sys::HtmlSelectElement;
use yew::prelude::*;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Deserialize)]
enum RunState {
    #[default]
    Unknown,
    Stopped,
    Running,
    Test,
}

#[derive(Debug, Default, PartialEq, Eq, Deserialize)]
struct Language {
    options: Vec<String>,
    current: String,
}

#[derive(Debug, Default, PartialEq, Eq, Deserialize)]
struct Wordlist {
    options: Vec<String>,
    current: Option<String>,
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
        <form {onsubmit} class="controls">
            { format!("Captions: {:?}; IP: {}; ", *run_state, *ip) }
            <button onclick={start}>{ "Start" }</button>
            <button onclick={stop}>{ "Stop" }</button>
            <button onclick={simulate}>{ "Test" }</button>

            <LanguageSelection />
            <WordlistSelection />
        </form>
    }
}

#[function_component]
fn LanguageSelection() -> Html {
    let language = use_state_eq(Language::default);

    wasm_bindgen_futures::spawn_local({
        let language = language.clone();
        async move {
            let new_language = Request::get("/api/lang")
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            language.set(new_language);
        }
    });

    let onchange = {
        let language = language.clone();
        move |new: Event| {
            let target: HtmlSelectElement =
                new.target().unwrap().dyn_into().unwrap();
            let new_lang = target.value().to_string();
            gloo::console::log!(&new_lang);

            wasm_bindgen_futures::spawn_local({
                let language = language.clone();
                async move {
                    let new_language = Request::post("/api/lang")
                        .json(&new_lang)
                        .unwrap()
                        .send()
                        .await
                        .unwrap()
                        .json()
                        .await
                        .unwrap();
                    language.set(new_language);
                }
            });
        }
    };

    let options = language
        .options
        .iter()
        .map(|option| {
            let selected = *option == language.current;
            html! {
                <option value={option.to_string()} {selected}>
                    { option }
                </option>
            }
        })
        .collect::<Html>();

    html! {
        <>
             { " Language: " }
            <select {onchange}>
                {options}
            </select>
        </>
    }
}

#[function_component]
fn WordlistSelection() -> Html {
    const SPECIAL_VALUE_FOR_NONE: &str = "special-value-for-none";

    let wordlist = use_state_eq(Wordlist::default);

    wasm_bindgen_futures::spawn_local({
        let wordlist = wordlist.clone();
        async move {
            let new_wordlist = Request::get("/api/wordlist")
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            wordlist.set(new_wordlist);
        }
    });

    let onchange = {
        let wordlist = wordlist.clone();
        move |new: Event| {
            let target: HtmlSelectElement =
                new.target().unwrap().dyn_into().unwrap();
            let new_wordlist = target.value().to_string();
            let new_wordlist = (new_wordlist != SPECIAL_VALUE_FOR_NONE)
                .then_some(new_wordlist);
            gloo::console::log!(format!("{new_wordlist:?}"));

            wasm_bindgen_futures::spawn_local({
                let wordlist = wordlist.clone();
                async move {
                    let new_wordlist = Request::post("/api/wordlist")
                        .json(&new_wordlist)
                        .unwrap()
                        .send()
                        .await
                        .unwrap()
                        .json()
                        .await
                        .unwrap();
                    wordlist.set(new_wordlist);
                }
            });
        }
    };

    let options = wordlist
        .options
        .iter()
        .map(|option| {
            let selected = Some(option) == wordlist.current.as_ref();
            html! {
                <option value={option.to_string()} {selected}>
                    { option }
                </option>
            }
        })
        .collect::<Html>();

    html! {
        <>
             { " Wordlist: " }
            <select {onchange}>
                <option
                    value={SPECIAL_VALUE_FOR_NONE}
                    selected={ wordlist.current.is_none() }
                >{"None"}</option>
                {options}
            </select>
        </>
    }
}

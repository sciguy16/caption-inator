use yew::prelude::*;

#[function_component]
pub fn Controls() -> Html {
    let onsubmit = |evt: SubmitEvent| {
        evt.prevent_default();
    };

    html! {
        <form {onsubmit}>
            <button>{ "Start" }</button>
            <button>{ "Stop" }</button>
            <button>{ "Test" }</button>
            { " Language: " }
            <select>
                <option>{ "English" }</option>
                <option>{ "English (IE)" }</option>
                <option>{ "English (US)" }</option>
                <option>{ "Japanese" }</option>
            </select>
        </form>
    }
}

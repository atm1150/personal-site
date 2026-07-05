/// This is the entry point for the wasm based frontend. It sets up the wasm application logging and starts the process of reading the page and mapping out the DOM to the our components
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use app::*;
    // initializes logging using the `log` crate
    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();

    leptos::mount::hydrate_body(App);
}

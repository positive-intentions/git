//! Git gallery (Dioxus + whatsup-ui chrome).
//!
//! ```bash
//! dx serve --bin git-gallery --platform web
//! # or
//! dx serve --bin git-gallery --platform desktop
//! ```

mod app;
mod chrome;
mod stories;

fn main() {
    dioxus::launch(app::App);
}

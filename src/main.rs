mod app;
mod config;
mod domain;
mod handlers;
mod items;
mod ui;
mod utils;
mod window_state;

use app::PantryApp;

fn main() {
    let app = PantryApp::new();
    app.run();
}

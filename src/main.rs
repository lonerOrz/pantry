mod app;
mod config;
mod constants;
mod domain;
mod handlers;
mod items;
mod services;
mod ui;
mod utils;
mod window_state;

use app::PantryApp;

fn main() {
    let app = PantryApp::new();
    app.run();
}

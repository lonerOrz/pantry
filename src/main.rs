mod app;
mod cache;
mod config;
mod constants;
mod domain;
mod services;
mod ui;
mod utils;
mod window_state;

use app::PantryApp;

fn main() {
    if std::env::var("GSK_RENDERER").is_err() {
        // SAFETY: called before any threads are spawned
        unsafe { std::env::set_var("GSK_RENDERER", "gl") };
    }
    let app = PantryApp::new();
    app.run();
}

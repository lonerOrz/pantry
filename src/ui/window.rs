use gtk4::{Application, ApplicationWindow, CssProvider, prelude::WidgetExt};

pub fn create_main_window(app: &Application) -> ApplicationWindow {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("pantry")
        .default_width(crate::constants::DEFAULT_WINDOW_WIDTH)
        .default_height(crate::constants::DEFAULT_WINDOW_HEIGHT)
        .resizable(true)
        .modal(true)
        .decorated(false)
        .build();

    let provider = CssProvider::new();
    provider.load_from_string(include_str!("../style.css"));
    gtk4::style_context_add_provider_for_display(
        &window.display(),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    window
}

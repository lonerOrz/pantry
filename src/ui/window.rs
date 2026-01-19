use gtk4::{prelude::WidgetExt, Application, ApplicationWindow, CssProvider};

pub fn create_main_window(app: &Application, _args: &crate::app::app::Args) -> ApplicationWindow {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("pantry")
        .default_width(1200)
        .default_height(800)
        .resizable(true)
        .modal(true)
        .decorated(false)
        .build();

    let provider = CssProvider::new();
    provider.load_from_data(include_str!("../style.css"));
    gtk4::style_context_add_provider_for_display(
        &window.display(),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    window
}

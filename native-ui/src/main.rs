// Release builds are GUI programs: no console window next to the app.
// Debug builds keep the console so local runs still show the logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod format;
mod i18n;
mod icons;
mod palette;
mod views;

use iced::window;

fn main() -> iced::Result {
    let options = app::Options::from_env();
    iced::application("Joinery Tools", app::update, views::view)
        .theme(|state: &app::App| state.iced_theme())
        .subscription(app::subscription)
        .window(window::Settings {
            size: (1160.0, 760.0).into(),
            min_size: Some((940.0, 620.0).into()),
            position: window::Position::Centered,
            ..Default::default()
        })
        .run_with(move || app::App::new(options))
}

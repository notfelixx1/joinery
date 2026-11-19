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

    // Window memory: the saved geometry is applied before the window
    // exists, straight from the settings database.
    let (position, size) = {
        let db = joinery_tools::storage::database::open(&app::settings_db_path()).ok();
        let read = |key: &str| -> Option<f32> {
            let conn = db.as_ref()?;
            joinery_tools::storage::database::get_setting(conn, key)
                .unwrap_or(None)?
                .parse()
                .ok()
        };
        let (x, y, w, h) = (read("win_x"), read("win_y"), read("win_w"), read("win_h"));
        let position = match (x, y) {
            (Some(x), Some(y)) if x.is_finite() && y.is_finite() => {
                window::Position::Specific(iced::Point::new(x, y))
            }
            _ => window::Position::Centered,
        };
        let size = match (w, h) {
            (Some(w), Some(h)) if w >= 300.0 && h >= 200.0 => (w, h).into(),
            _ => (1160.0, 760.0).into(),
        };
        (position, size)
    };

    iced::application("Joinery Tools", app::update, views::view)
        .theme(|state: &app::App| state.iced_theme())
        .subscription(app::subscription)
        .window(window::Settings {
            size,
            min_size: Some((940.0, 620.0).into()),
            position,
            ..Default::default()
        })
        .run_with(move || app::App::new(options))
}

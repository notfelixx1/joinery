mod app;
mod format;
mod i18n;
mod icons;
mod palette;
mod views;

pub use format::format_bytes;

use iced::window;

fn main() -> iced::Result {
    iced::application("Joinery Tools", app::update, views::view)
        .theme(|state: &app::App| state.iced_theme())
        .subscription(app::subscription)
        .window(window::Settings {
            size: (1160.0, 760.0).into(),
            min_size: Some((940.0, 620.0).into()),
            position: window::Position::Centered,
            ..Default::default()
        })
        .run_with(app::App::new)
}

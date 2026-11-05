use iced::widget::{column, row, text};
use iced::{font, Element, Length};

use crate::app::{App, Msg};
use crate::i18n::tr;
use crate::views::{card, muted};

pub fn view(app: &App) -> Element<'_, Msg> {
    let dark = app.dark;
    let lang = app.lang;

    let os = match std::env::consts::OS {
        "windows" => "Windows",
        "linux" => "Linux",
        "macos" => "macOS",
        other => other,
    };

    let info = column![
        info_row(lang, dark, "system.os", os.to_string()),
        info_row(
            lang,
            dark,
            "system.arch",
            std::env::consts::ARCH.to_string()
        ),
        info_row(
            lang,
            dark,
            "system.version",
            format!("Joinery Tools v{} (native)", env!("CARGO_PKG_VERSION"))
        ),
    ]
    .spacing(0)
    .width(Length::Fill);

    let mut col = column![].spacing(16).width(Length::Fill);
    col = col.push(card(
        dark,
        column![text(tr(lang, "system.info")).size(15), info,]
            .spacing(8)
            .width(Length::Fill),
    ));
    col = col.push(card(
        dark,
        row![
            text("v0.2")
                .size(12)
                .font(font::Font::MONOSPACE)
                .style(muted(dark)),
            text(tr(lang, "system.coming"))
                .size(12)
                .style(muted(dark))
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::Word),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    ));

    col.into()
}

fn info_row(
    lang: crate::i18n::Lang,
    dark: bool,
    key: &'static str,
    value: String,
) -> Element<'static, Msg> {
    row![
        text(tr(lang, key))
            .size(13)
            .style(muted(dark))
            .width(Length::Fixed(220.0)),
        text(value).size(13),
    ]
    .padding([8, 0])
    .into()
}

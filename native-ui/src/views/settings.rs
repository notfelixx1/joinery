//! Settings page: two groups — Appearance (accent color, theme,
//! language) and Functionality (checksum auto-copy).

use iced::widget::{button, checkbox, column, container, row, text, text_input};
use iced::{font, Alignment, Element, Length};

use super::{card_style, muted};
use crate::app::checksum::Msg as CksMsg;
use crate::app::{App, Msg};
use crate::i18n::{tr, Lang};
use crate::palette;

fn section_title(lang: Lang, dark: bool, key: &str) -> Element<'static, Msg> {
    text(tr(lang, key))
        .size(14)
        .font(font::Font {
            weight: font::Weight::Semibold,
            ..font::Font::default()
        })
        .style(muted(dark))
        .into()
}

fn card<'a>(dark: bool, content: Element<'a, Msg>) -> Element<'a, Msg> {
    container(content)
        .width(Length::Fill)
        .padding(16)
        .style(move |_| card_style(dark))
        .into()
}

fn swatch(dark: bool, color: iced::Color, selected: bool, msg: Msg) -> Element<'static, Msg> {
    let s = palette::scheme(dark);
    let border_color = if selected { s.text } else { s.divider };
    button(
        container(iced::widget::Space::with_width(Length::Fixed(26.0))).height(Length::Fixed(22.0)),
    )
    .style(move |_, _| iced::widget::button::Style {
        background: Some(color.into()),
        border: iced::Border {
            color: border_color,
            width: if selected { 2.0 } else { 1.0 },
            radius: 6.0.into(),
        },
        ..Default::default()
    })
    .on_press(msg)
    .into()
}

pub fn view(app: &App) -> Element<'_, Msg> {
    let dark = app.dark;
    let lang = app.lang;
    let s = palette::scheme(dark);
    let accent = palette::accent();

    // ---- appearance ----
    let mut presets = row![].spacing(8).align_y(Alignment::Center);
    for (_, color) in palette::ACCENT_PRESETS {
        presets = presets.push(swatch(
            dark,
            color,
            color == accent,
            Msg::AccentPreset(color),
        ));
    }

    let hex_valid = palette::parse_hex(&app.accent_input).is_some();
    // One hint, two moods: muted while the hex parses, danger while it
    // does not.
    let hint_color = if hex_valid { s.text_muted } else { s.danger };
    let hint_style = move |_: &iced::Theme| iced::widget::text::Style {
        color: Some(hint_color),
    };

    let accent_row = row![
        presets,
        text_input("", &app.accent_input)
            .on_input(Msg::AccentInput)
            .width(Length::Fixed(150.0))
            .size(13)
            .font(font::Font::MONOSPACE),
        container(iced::widget::Space::with_width(Length::Fixed(26.0)))
            .height(Length::Fixed(22.0))
            .style(move |_| container::Style {
                background: Some(accent.into()),
                border: iced::Border {
                    color: s.divider,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            }),
        text(tr(lang, "settings.accent.hint"))
            .size(11)
            .style(hint_style),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let theme_row = row![
        text(tr(lang, "settings.dark_theme")).size(13),
        iced::widget::horizontal_space(),
        checkbox("", app.dark).on_toggle(Msg::SetDark).size(14),
    ]
    .width(Length::Fill)
    .align_y(Alignment::Center);

    let language_row = row![
        text(tr(lang, "settings.language")).size(13),
        iced::widget::horizontal_space(),
        iced::widget::pick_list(Lang::ALL, Some(app.lang), Msg::SetLang)
            .text_size(13)
            .width(Length::Shrink),
    ]
    .width(Length::Fill)
    .align_y(Alignment::Center);

    let appearance = card(
        dark,
        column![
            section_title(lang, dark, "settings.appearance"),
            accent_row,
            theme_row,
            language_row,
        ]
        .spacing(14)
        .width(Length::Fill)
        .into(),
    );

    // ---- functionality ----
    let functionality = card(
        dark,
        column![
            section_title(lang, dark, "settings.functionality"),
            row![
                text(tr(lang, "checksum.autocopy")).size(13),
                iced::widget::horizontal_space(),
                checkbox("", app.cks_autocopy)
                    .on_toggle(|v| Msg::Checksum(CksMsg::Autocopy(v)))
                    .size(14),
            ]
            .width(Length::Fill)
            .align_y(Alignment::Center),
        ]
        .spacing(14)
        .width(Length::Fill)
        .into(),
    );

    column![appearance, functionality]
        .spacing(16)
        .width(Length::Fill)
        .into()
}

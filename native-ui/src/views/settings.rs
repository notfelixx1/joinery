//! Settings page: two groups — Appearance (accent color, theme,
//! language) and Functionality (checksum auto-copy).

use iced::widget::{checkbox, column, container, row, text, text_input};
use iced::{font, Alignment, Element, Length};

use super::{btn, card_style, checkbox_style, muted, pick_field_style, pick_menu_style};
use crate::app::checksum::Msg as CksMsg;
use crate::app::{App, Msg};
use crate::i18n::{tr, Lang};
use crate::palette;

fn section_title(lang: Lang, key: &str) -> Element<'static, Msg> {
    text(tr(lang, key))
        .size(14)
        .font(font::Font {
            weight: font::Weight::Semibold,
            ..font::Font::default()
        })
        .style(muted())
        .into()
}

fn card<'a>(content: Element<'a, Msg>) -> Element<'a, Msg> {
    container(content)
        .width(Length::Fill)
        .padding(16)
        .style(move |_| card_style())
        .into()
}

pub fn view(app: &App) -> Element<'_, Msg> {
    let lang = app.lang;
    let s = palette::scheme();

    // ---- appearance ----
    let presets = row![
        btn(
            tr(lang, "settings.pal.dark"),
            Msg::PalettePreset(crate::palette::UserColors::default()),
        ),
        btn(
            tr(lang, "settings.pal.light"),
            Msg::PalettePreset(crate::palette::UserColors::light()),
        ),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let mut color_rows = column![].spacing(10).width(Length::Fill);
    for slot in palette::ColorSlot::ALL {
        let value = app
            .color_inputs
            .get(&slot)
            .cloned()
            .unwrap_or_else(|| palette::to_hex(slot.get(&palette::user_colors())));
        let parsed = palette::parse_hex(&value);
        let preview_color = parsed.unwrap_or(s.divider);
        let normalized = parsed
            .map(palette::to_hex)
            .unwrap_or_else(|| "#??????".to_string());
        let valid = parsed.is_some();
        let hint_color = if valid && value == palette::to_hex(slot.get(&palette::user_colors())) {
            s.text_muted
        } else if valid {
            s.ok
        } else {
            s.danger
        };
        color_rows = color_rows.push(
            row![
                text(tr(lang, slot.i18n_key())).size(13),
                iced::widget::horizontal_space(),
                text_input("", &value)
                    .on_input(move |v| Msg::ColorInput(slot, v))
                    .width(Length::Fixed(120.0))
                    .size(13)
                    .font(font::Font::MONOSPACE),
                container(iced::widget::Space::with_width(Length::Fixed(24.0)))
                    .height(Length::Fixed(20.0))
                    .style(move |_| container::Style {
                        background: Some(preview_color.into()),
                        border: iced::Border {
                            color: s.divider,
                            width: 1.0,
                            radius: 5.0.into(),
                        },
                        ..Default::default()
                    }),
                text(normalized)
                    .size(10)
                    .font(font::Font::MONOSPACE)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(hint_color),
                    }),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .width(Length::Fill),
        );
    }

    let language_row = row![
        text(tr(lang, "settings.language")).size(13),
        iced::widget::horizontal_space(),
        iced::widget::pick_list(Lang::ALL, Some(app.lang), Msg::SetLang)
            .text_size(13)
            .width(Length::Shrink)
            .style(pick_field_style)
            .menu_style(pick_menu_style),
    ]
    .width(Length::Fill)
    .align_y(Alignment::Center);

    let appearance = card(
        column![
            section_title(lang, "settings.appearance"),
            presets,
            color_rows,
            language_row,
        ]
        .spacing(14)
        .width(Length::Fill)
        .into(),
    );

    // ---- functionality ----
    let autocopy_row: Element<Msg> = row![
        text(tr(lang, "checksum.autocopy")).size(13),
        iced::widget::horizontal_space(),
        checkbox("", app.cks_autocopy)
            .on_toggle(|v| Msg::Checksum(CksMsg::Autocopy(v)))
            .size(14)
            .style(checkbox_style),
    ]
    .width(Length::Fill)
    .align_y(Alignment::Center)
    .into();

    let functionality: Element<Msg> = card(
        column![section_title(lang, "settings.functionality"), autocopy_row,]
            .spacing(14)
            .width(Length::Fill)
            .into(),
    );

    column![appearance, functionality]
        .spacing(16)
        .width(Length::Fill)
        .into()
}

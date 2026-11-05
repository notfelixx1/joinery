use iced::widget::{column, container, row, text};
use iced::{Alignment, Element, Length};

use crate::app::{App, Msg};
use crate::i18n::tr;
use crate::palette;
use crate::views::{card, muted, text_color};

pub fn view(app: &App) -> Element<'_, Msg> {
    let dark = app.dark;
    let lang = app.lang;
    let s = palette::scheme(dark);

    let amount = column![
        text("$5").size(28).style(text_color(s.accent)),
        text(tr(lang, "support.amountNote"))
            .size(12)
            .style(muted(dark)),
        column![
            point("support.point1", lang),
            point("support.point2", lang),
            point("support.point3", lang),
        ]
        .spacing(4)
        .padding(iced::Padding {
            top: 12.0,
            ..Default::default()
        }),
        text(tr(lang, "support.noPressure"))
            .size(12)
            .style(muted(dark))
            .wrapping(iced::widget::text::Wrapping::Word),
    ]
    .spacing(12)
    .width(Length::Fill);

    let links = column![
        link_row("support.sponsors", "github.com/sponsors", lang, dark),
        link_row("support.kofi", "ko-fi.com", lang, dark),
        link_row("support.bmc", "buymeacoffee.com", lang, dark),
    ]
    .spacing(8);

    column![card(
        dark,
        column![
            text(tr(lang, "support.title")).size(20),
            text(tr(lang, "support.subtitle"))
                .size(13)
                .style(muted(dark)),
            container(amount)
                .width(Length::Fixed(480.0))
                .padding(20)
                .style(move |_| iced::widget::container::Style {
                    background: None,
                    text_color: Some(s.text),
                    border: iced::Border {
                        color: s.border,
                        width: 1.0,
                        radius: 12.0.into(),
                    },
                    ..Default::default()
                }),
            links,
        ]
        .spacing(12)
        .width(Length::Fill),
    ),]
    .spacing(16)
    .width(Length::Fill)
    .into()
}

fn point(key: &'static str, lang: crate::i18n::Lang) -> Element<'static, Msg> {
    text(format!("• {}", tr(lang, key)))
        .size(13)
        .wrapping(iced::widget::text::Wrapping::Word)
        .into()
}

fn link_row(
    key: &'static str,
    url: &'static str,
    lang: crate::i18n::Lang,
    dark: bool,
) -> Element<'static, Msg> {
    row![
        text(tr(lang, key)).size(13),
        text(url).size(12).style(muted(dark)),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

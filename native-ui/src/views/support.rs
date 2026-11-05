use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Color, Element, Length};

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

    let links = column![link_row(
        "Ko-fi",
        "ko-fi.com/notfst",
        "https://ko-fi.com/notfst",
        dark,
    )]
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

/// One donation link: the whole row is a button that opens the browser.
fn link_row(
    label: &'static str,
    display_url: &'static str,
    open_url: &'static str,
    dark: bool,
) -> Element<'static, Msg> {
    button(
        row![
            text(label).size(13),
            text(display_url).size(12).style(muted(dark)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([6, 10])
    .style(move |_, status| {
        let mut st = button::Style {
            background: Some(Color::TRANSPARENT.into()),
            text_color: palette::scheme(dark).text,
            border: iced::Border {
                color: palette::scheme(dark).border,
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow: Default::default(),
        };
        if let button::Status::Hovered = status {
            st.background = Some(palette::scheme(dark).surface_hover.into());
            st.border.color = palette::scheme(dark).accent;
        }
        st
    })
    .on_press(Msg::OpenUrl(open_url))
    .into()
}

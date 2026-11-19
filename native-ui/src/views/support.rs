use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length};

use crate::app::{App, Msg};
use crate::i18n::tr;
use crate::palette;
use crate::views::{card, muted, text_color};

pub fn view(app: &App) -> Element<'_, Msg> {
    let lang = app.lang;
    let s = palette::scheme();

    let amount = column![
        text("$5").size(28).style(text_color(s.accent)),
        text(tr(lang, "support.amountNote")).size(12).style(muted()),
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
            .style(muted())
            .wrapping(iced::widget::text::Wrapping::Word),
    ]
    .spacing(12)
    .width(Length::Fill);

    let links = container(column![link_row(
        crate::icons::heart(),
        "Ko-fi",
        "ko-fi.com/notfst",
        "https://ko-fi.com/notfst",
    )])
    .width(Length::Fill)
    .align_x(Alignment::Center);

    column![card(
        column![
            text(tr(lang, "support.title")).size(20),
            text(tr(lang, "support.subtitle")).size(13).style(muted()),
            container(amount)
                .width(Length::Fixed(480.0))
                .padding(20)
                .align_x(Alignment::Center)
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
    icon: iced::widget::svg::Svg<'static>,
    label: &'static str,
    display_url: &'static str,
    open_url: &'static str,
) -> Element<'static, Msg> {
    let s = palette::scheme();
    button(
        row![
            icon.width(Length::Fixed(16.0))
                .height(Length::Fixed(16.0))
                .style(move |_, _| iced::widget::svg::Style {
                    color: Some(s.accent_text),
                },),
            text(label).size(13),
            text(display_url).size(12).style(muted()),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([10, 18])
    .style(move |_, status| {
        let hovered = matches!(status, button::Status::Hovered);
        button::Style {
            background: if hovered {
                Some(palette::mix(s.accent, s.text, 0.10).into())
            } else {
                Some(s.accent.into())
            },
            text_color: s.accent_text,
            border: iced::Border {
                color: if hovered { s.text } else { s.accent },
                width: 1.0,
                radius: 10.0.into(),
            },
            shadow: Default::default(),
        }
    })
    .on_press(Msg::OpenUrl(open_url))
    .into()
}

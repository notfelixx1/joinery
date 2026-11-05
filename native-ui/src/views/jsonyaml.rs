use iced::widget::{column, row, scrollable, text, text_editor};
use iced::{font, Alignment, Element, Length};

use crate::app::{App, JyKind, Msg};
use crate::i18n::{fmt, tr};
use crate::palette;
use crate::views::{btn, btn_primary, card, error_box, muted, text_color};

pub fn view(app: &App) -> Element<'_, Msg> {
    let dark = app.dark;
    let lang = app.lang;
    let j = &app.jy;

    let col = column![].spacing(16).width(Length::Fill);

    let kinds = [JyKind::Json, JyKind::Yaml];

    let mut top = column![].spacing(12).width(Length::Fill);
    top = top.push(
        row![
            iced::widget::pick_list(kinds, Some(j.kind.clone()), Msg::JyKind).text_size(13),
            iced::widget::horizontal_space(),
            btn(dark, tr(lang, "common.example"), Msg::JyExample),
            btn(dark, tr(lang, "common.clear"), Msg::JyClear),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    );

    let input = text_editor(&j.content)
        .height(Length::Fixed(280.0))
        .on_action(Msg::JyEdit)
        .placeholder("{ \"joinery\": true }")
        .size(13);

    let output_text = match &j.result {
        Some(result) if result.valid => result.formatted.clone().unwrap_or_default(),
        _ => String::new(),
    };

    let output = scrollable(
        container_text(output_text, dark),
    )
    .height(Length::Fixed(280.0))
    .width(Length::Fill);

    top = top.push(
        row![
            column![text(tr(lang, "dev.input")).size(12).style(muted(dark)), input]
                .spacing(4)
                .width(Length::Fill),
            column![text(tr(lang, "dev.output")).size(12).style(muted(dark)), output]
                .spacing(4)
                .width(Length::Fill),
        ]
        .spacing(12)
        .width(Length::Fill),
    );

    let mut actions = row![
        btn_primary(
            dark,
            &if j.busy {
                tr(lang, "common.loading").to_string()
            } else {
                tr(lang, "common.validate").to_string()
            },
            if !j.busy && !j.content.text().trim().is_empty() {
                Some(Msg::JyValidate)
            } else {
                None
            },
        ),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    if j.kind == JyKind::Json {
        actions = actions
            .push(btn(dark, tr(lang, "common.format"), Msg::JyReformat(false)))
            .push(btn(dark, tr(lang, "common.minify"), Msg::JyReformat(true)));
    }
    actions = actions.push(iced::widget::horizontal_space());
    if let Some(result) = &j.result {
        let color = if result.valid {
            palette::scheme(dark).ok
        } else {
            palette::scheme(dark).danger
        };
        let key = if result.valid { "dev.valid" } else { "dev.invalid" };
        actions = actions.push(
            text(fmt(tr(lang, key), &[("kind", j.kind.to_string())]))
                .size(12)
                .style(text_color(color)),
        );
    }
    top = top.push(actions);

    if let Some(result) = &j.result {
        if !result.valid {
            if let Some(err) = &result.error {
                let mut message = err.message.clone();
                if let Some(line) = err.line {
                    message.push('\n');
                    message.push_str(&fmt(
                        tr(lang, "dev.errorAt"),
                        &[
                            ("line", line.to_string()),
                            ("column", err.column.unwrap_or(0).to_string()),
                        ],
                    ));
                }
                top = top.push(error_box(dark, &message));
            }
        }
    }
    if let Some(err) = &j.error {
        top = top.push(error_box(dark, err));
    }

    col.push(card(dark, top)).into()
}

fn container_text(content: String, dark: bool) -> Element<'static, Msg> {
    let s = palette::scheme(dark);
    if content.is_empty() {
        text("")
            .size(13)
            .width(Length::Fill)
            .into()
    } else {
        text(content)
            .size(12)
            .font(font::Font::MONOSPACE)
            .style(text_color(s.text))
            .width(Length::Fill)
            .wrapping(iced::widget::text::Wrapping::Word)
            .into()
    }
}

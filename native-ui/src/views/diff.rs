use iced::widget::{column, row, scrollable, text, text_editor};
use iced::{font, Alignment, Element, Length};

use joinery_tools::core::text_diff::DiffKind;

use crate::app::{App, Msg};
use crate::i18n::{fmt, tr};
use crate::palette;
use crate::views::{btn, btn_primary, card, error_box, muted, text_color};

pub fn view(app: &App) -> Element<'_, Msg> {
    let dark = app.dark;
    let lang = app.lang;
    let d = &app.diff;
    let s = palette::scheme(dark);

    let mut col = column![].spacing(16).width(Length::Fill);

    let old_pane = column![
        row![
            text(tr(lang, "dev.oldText"))
                .size(12)
                .style(muted(dark))
                .width(Length::Fill),
            btn(dark, tr(lang, "dev.loadFile"), Msg::DiffLoad(true)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        text_editor(&d.old_text)
            .height(Length::Fixed(220.0))
            .on_action(|a| Msg::DiffEdit(true, a))
            .size(13),
    ]
    .spacing(4)
    .width(Length::Fill);

    let new_pane = column![
        row![
            text(tr(lang, "dev.newText"))
                .size(12)
                .style(muted(dark))
                .width(Length::Fill),
            btn(dark, tr(lang, "dev.loadFile"), Msg::DiffLoad(false)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        text_editor(&d.new_text)
            .height(Length::Fixed(220.0))
            .on_action(|a| Msg::DiffEdit(false, a))
            .size(13),
    ]
    .spacing(4)
    .width(Length::Fill);

    let mut controls = column![].spacing(12).width(Length::Fill);
    controls = controls.push(row![old_pane, new_pane].spacing(12).width(Length::Fill));

    let mut actions = row![
        btn_primary(
            dark,
            &if d.busy {
                tr(lang, "common.loading").to_string()
            } else {
                tr(lang, "common.compare").to_string()
            },
            if !d.busy {
                Some(Msg::DiffRun)
            } else {
                None
            },
        ),
        btn(dark, tr(lang, "common.clear"), Msg::DiffClear),
        iced::widget::horizontal_space(),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    if let Some(result) = &d.result {
        actions = actions
            .push(
                text(fmt(tr(lang, "dev.added"), &[("n", result.stats.added.to_string())]))
                    .size(12)
                    .font(font::Font::MONOSPACE)
                    .style(text_color(s.ok)),
            )
            .push(
                text(fmt(tr(lang, "dev.removed"), &[("n", result.stats.removed.to_string())]))
                    .size(12)
                    .font(font::Font::MONOSPACE)
                    .style(text_color(s.danger)),
            )
            .push(
                text(fmt(tr(lang, "dev.unchanged"), &[("n", result.stats.unchanged.to_string())]))
                    .size(12)
                    .font(font::Font::MONOSPACE)
                    .style(text_color(s.text_muted)),
            );
    }
    controls = controls.push(actions);

    if let Some(err) = &d.error {
        controls = controls.push(error_box(dark, err));
    }

    col = col.push(card(dark, controls));

    if let Some(result) = &d.result {
        let mut rows = column![].spacing(0).width(Length::Fill);
        for row_data in &result.rows {
            let (sign, color) = match row_data.kind {
                DiffKind::Add => ("+ ", s.ok),
                DiffKind::Delete => ("− ", s.danger),
                DiffKind::Equal => ("  ", s.text_muted),
            };
            rows = rows.push(
                row![
                    text(row_data
                        .old_line
                        .map(|n| n.to_string())
                        .unwrap_or_default())
                    .size(11)
                    .font(font::Font::MONOSPACE)
                    .style(muted(dark))
                    .width(Length::Fixed(40.0)),
                    text(row_data
                        .new_line
                        .map(|n| n.to_string())
                        .unwrap_or_default())
                    .size(11)
                    .font(font::Font::MONOSPACE)
                    .style(muted(dark))
                    .width(Length::Fixed(40.0)),
                    text(format!("{}{}", sign, row_data.text))
                        .size(12)
                        .font(font::Font::MONOSPACE)
                        .style(text_color(color))
                        .width(Length::Fill)
                        .wrapping(iced::widget::text::Wrapping::Word),
                ]
                .spacing(4)
                .padding([2, 8]),
            );
        }
        col = col.push(card(
            dark,
            scrollable(rows).height(Length::Fixed(420.0)),
        ));
    }

    col.into()
}

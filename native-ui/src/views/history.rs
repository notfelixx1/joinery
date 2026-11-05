//! Undo journal listing: every applied batch rename, one Undo per row.

use iced::widget::{column, row, scrollable, text};
use iced::{font, Alignment, Element, Length};

use crate::app::{App, Msg};
use crate::i18n::{fmt, tr};
use crate::palette;
use crate::views::{btn_danger, card, empty_state, muted, text_color};

pub fn view(app: &App) -> Element<'_, Msg> {
    let dark = app.dark;
    let lang = app.lang;

    if app.history.is_empty() {
        return empty_state(
            dark,
            crate::icons::clock,
            tr(lang, "nav.history"),
            tr(lang, "history.empty"),
            None,
        );
    }

    let mut col = column![].spacing(16).width(Length::Fill);
    if let Some(note) = &app.history_note {
        col = col.push(
            text(note.to_string())
                .size(12)
                .style(text_color(palette::scheme(dark).danger)),
        );
    }

    let mut rows = column![].spacing(0).width(Length::Fill);
    rows = rows.push(
        row![
            text(tr(lang, "history.date"))
                .size(12)
                .style(muted(dark))
                .width(Length::Fixed(120.0)),
            text(tr(lang, "history.operation"))
                .size(12)
                .style(muted(dark))
                .width(Length::Fill),
            text(tr(lang, "common.files"))
                .size(12)
                .style(muted(dark))
                .width(Length::Fixed(100.0)),
            text("").size(12).width(Length::Fixed(110.0)),
        ]
        .spacing(10)
        .padding([8, 8]),
    );

    for entry in &app.history {
        let when = chrono::DateTime::parse_from_rfc3339(&entry.created_at)
            .map(|dt| {
                dt.with_timezone(&chrono::Local)
                    .format("%d.%m.%Y %H:%M")
                    .to_string()
            })
            .unwrap_or_else(|_| entry.created_at.clone());
        let operation = if entry.tool == "renamer" {
            tr(lang, "tool.renamer").to_string()
        } else {
            entry.tool.clone()
        };
        let action: Element<'_, Msg> = if entry.undone {
            text(tr(lang, "history.undone"))
                .size(12)
                .style(muted(dark))
                .width(Length::Fixed(110.0))
                .into()
        } else {
            btn_danger(
                dark,
                tr(lang, "common.undo"),
                Some(Msg::UndoJournal(entry.id)),
            )
        };
        rows = rows.push(
            row![
                text(when)
                    .size(12)
                    .font(font::Font::MONOSPACE)
                    .style(muted(dark))
                    .width(Length::Fixed(120.0)),
                text(operation).size(13).width(Length::Fill),
                text(fmt(
                    tr(lang, "history.files"),
                    &[("n", entry.op_count.to_string())]
                ))
                .size(12)
                .style(muted(dark))
                .width(Length::Fixed(100.0)),
                action,
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .padding([6, 8]),
        );
    }

    col = col.push(card(dark, scrollable(rows).height(Length::Fixed(520.0))));
    col.into()
}

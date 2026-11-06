use iced::widget::{checkbox, column, row, scrollable, text, text_input};
use iced::{font, Alignment, Element, Length};

use joinery_tools::core::renamer::RenameStatus;

use crate::app::renamer::Msg as RenMsg;
use crate::app::{App, Msg};
use crate::i18n::{fmt, tr};
use crate::palette;
use crate::views::{
    btn, btn_danger, btn_primary, card, empty_state, error_box, field, muted, ok_box, recent_block,
    text_color,
};

pub fn view(app: &App) -> Element<'_, Msg> {
    let dark = app.dark;
    let lang = app.lang;
    let r = &app.ren;
    let s = palette::scheme(dark);

    if r.folder.is_none() {
        let mut col = column![].spacing(16).width(Length::Fill);
        col = col.push(empty_state(
            dark,
            crate::icons::tag,
            tr(lang, "tool.renamer"),
            tr(lang, "empty.renamer.hint"),
            Some((
                tr(lang, "common.chooseFolder"),
                Msg::Renamer(RenMsg::ChooseFolder),
            )),
        ));
        if !app.ren_recent.is_empty() {
            col = col.push(recent_block(
                dark,
                tr(lang, "empty.recent"),
                &app.ren_recent,
                |p| Msg::Renamer(RenMsg::Folder(Some(p))),
            ));
        }
        return col.into();
    }

    // The empty state already handled a missing folder — one is
    // guaranteed to be picked here.
    let folder_text = r
        .folder
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let left = column![
        field(
            dark,
            tr(lang, "renamer.find"),
            text_input("photo", &r.find)
                .on_input(|v| Msg::Renamer(RenMsg::Find(v)))
                .size(13),
        ),
        field(
            dark,
            tr(lang, "renamer.prefix"),
            text_input("", &r.prefix)
                .on_input(|v| Msg::Renamer(RenMsg::Prefix(v)))
                .size(13),
        ),
        field(
            dark,
            tr(lang, "renamer.newExt"),
            text_input("jpg", &r.new_ext)
                .on_input(|v| Msg::Renamer(RenMsg::NewExt(v)))
                .size(13),
        ),
    ]
    .spacing(12)
    .width(Length::Fill);

    let right = column![
        field(
            dark,
            tr(lang, "renamer.replace"),
            text_input("vacation", &r.replace)
                .on_input(|v| Msg::Renamer(RenMsg::Replace(v)))
                .size(13),
        ),
        field(
            dark,
            tr(lang, "renamer.suffix"),
            text_input("", &r.suffix)
                .on_input(|v| Msg::Renamer(RenMsg::Suffix(v)))
                .size(13),
        ),
        field(
            dark,
            tr(lang, "renamer.pattern"),
            text_input("photo_{n:3}", &r.pattern)
                .on_input(|v| Msg::Renamer(RenMsg::Pattern(v)))
                .size(13),
        ),
    ]
    .spacing(12)
    .width(Length::Fill);

    let top = card(
        dark,
        column![
            row![
                btn(
                    dark,
                    tr(lang, "common.chooseFolder"),
                    Msg::Renamer(RenMsg::ChooseFolder)
                ),
                text(folder_text)
                    .size(12)
                    .font(font::Font::MONOSPACE)
                    .style(muted(dark))
                    .width(Length::Fill)
                    .wrapping(iced::widget::text::Wrapping::Word),
                btn_primary(
                    dark,
                    &if r.busy {
                        tr(lang, "common.loading").to_string()
                    } else {
                        tr(lang, "common.preview").to_string()
                    },
                    if r.folder.is_some() && !r.busy {
                        Some(Msg::Renamer(RenMsg::Preview))
                    } else {
                        None
                    },
                ),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            row![left, right].spacing(12).width(Length::Fill),
            text(tr(lang, "renamer.pattern.hint"))
                .size(12)
                .style(muted(dark)),
            row![
                checkbox(tr(lang, "renamer.subfolders"), r.include_subfolders)
                    .on_toggle(|v| Msg::Renamer(RenMsg::Subfolders(v)))
                    .text_size(13),
                iced::widget::tooltip(
                    checkbox(tr(lang, "renamer.allowOverwrite"), r.allow_overwrite)
                        .on_toggle(|v| Msg::Renamer(RenMsg::AllowOverwrite(v)))
                        .text_size(13),
                    text(tr(lang, "renamer.allowOverwrite.hint")).size(12),
                    iced::widget::tooltip::Position::Top,
                ),
            ]
            .spacing(20),
        ]
        .spacing(12)
        .width(Length::Fill),
    );

    let mut col = column![].spacing(16).width(Length::Fill);
    col = col.push(top);

    if let Some(err) = &r.error {
        col = col.push(error_box(dark, err));
    }
    if let Some(note) = &r.note {
        col = col.push(ok_box(dark, note));
    }

    if let Some(plan) = &r.plan {
        let mut rows = column![].spacing(0).width(Length::Fill);

        rows = rows.push(
            row![
                text(tr(lang, "common.name"))
                    .size(11)
                    .style(muted(dark))
                    .width(Length::FillPortion(4)),
                text("→")
                    .size(11)
                    .style(muted(dark))
                    .width(Length::Fixed(24.0)),
                text(tr(lang, "common.name") /* target */)
                    .size(11)
                    .style(muted(dark))
                    .width(Length::FillPortion(2)),
                text(tr(lang, "common.status"))
                    .size(11)
                    .style(muted(dark))
                    .width(Length::Fixed(110.0)),
            ]
            .padding([8, 8]),
        );

        for item in plan {
            let (status_key, color) = match item.status {
                RenameStatus::Ready => ("renamer.status.ready", s.ok),
                RenameStatus::CaseOnly => ("renamer.status.caseOnly", s.accent),
                RenameStatus::NoChange => ("renamer.status.nochange", s.text_muted),
                RenameStatus::TargetExists => ("renamer.status.targetExists", s.warn),
                RenameStatus::BatchDuplicate => ("renamer.status.batchDuplicate", s.danger),
                RenameStatus::InvalidName => ("renamer.status.invalidName", s.danger),
            };
            let unchanged = item.status == RenameStatus::NoChange;
            rows = rows.push(
                row![
                    text(&item.from_name)
                        .size(12)
                        .font(font::Font::MONOSPACE)
                        .width(Length::FillPortion(4))
                        .wrapping(iced::widget::text::Wrapping::Word),
                    text("→")
                        .size(12)
                        .style(muted(dark))
                        .width(Length::Fixed(24.0)),
                    text(if unchanged {
                        "="
                    } else {
                        item.to_name.as_str()
                    })
                    .size(12)
                    .font(font::Font::MONOSPACE)
                    .style(text_color(if unchanged { s.text_muted } else { s.accent }))
                    .width(Length::FillPortion(2))
                    .wrapping(iced::widget::text::Wrapping::Word),
                    text(tr(lang, status_key))
                        .size(12)
                        .style(text_color(color))
                        .width(Length::Fixed(110.0)),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .padding([4, 8]),
            );
        }

        col = col.push(card(
            dark,
            column![
                text(tr(lang, "renamer.plan")).size(15),
                scrollable(rows).height(Length::Fixed(340.0)),
                row![
                    btn_primary(
                        dark,
                        &format!("{} ({})", tr(lang, "common.apply"), r.ready_count),
                        if !r.busy && r.ready_count > 0 && !r.has_conflicts {
                            Some(Msg::Renamer(RenMsg::Apply))
                        } else {
                            None
                        },
                    ),
                    text(if r.has_conflicts {
                        tr(lang, "renamer.status.batchDuplicate")
                    } else {
                        ""
                    })
                    .size(12)
                    .style(muted(dark)),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .padding(iced::Padding {
                    top: 12.0,
                    ..Default::default()
                }),
            ]
            .spacing(12)
            .width(Length::Fill),
        ));
    }

    if let Some((applied, skipped)) = r.outcome {
        let template = tr(lang, "renamer.applyDone");
        let summary = fmt(
            template,
            &[
                ("applied", applied.to_string()),
                ("skipped", skipped.to_string()),
            ],
        );
        col = col.push(card(
            dark,
            row![
                ok_box(dark, &summary),
                btn_danger(
                    dark,
                    tr(lang, "common.undo"),
                    if r.journal.is_some() && !r.busy {
                        Some(Msg::Renamer(RenMsg::Undo))
                    } else {
                        None
                    },
                ),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        ));
    }

    col.into()
}

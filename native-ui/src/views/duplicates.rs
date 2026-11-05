use iced::widget::{checkbox, column, row, scrollable, text, text_input};
use iced::{font, Alignment, Element, Length};

use crate::app::{App, Msg};
use crate::format_bytes;
use crate::i18n::{fmt, tr};
use crate::views::{btn, btn_danger, card, error_box, muted, ok_box};

pub fn view(app: &App) -> Element<'_, Msg> {
    let dark = app.dark;
    let lang = app.lang;
    let d = &app.dup;

    let folder_text = d
        .folder
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| tr(lang, "renamer.pickFolderFirst").to_string());

    let selected_count: usize = d
        .result
        .as_ref()
        .map(|r| {
            r.groups
                .iter()
                .flat_map(|g| g.files.iter())
                .filter(|f| {
                    d.selected
                        .get(&f.path.to_string_lossy().to_string())
                        .copied()
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0);

    let mut col = column![].spacing(16).width(Length::Fill);

    col = col.push(card(
        dark,
        column![
            row![
                btn(dark, tr(lang, "common.chooseFolder"), Msg::DupChooseFolder),
                text(folder_text)
                    .size(12)
                    .font(font::Font::MONOSPACE)
                    .style(muted(dark))
                    .width(Length::Fill)
                    .wrapping(iced::widget::text::Wrapping::Word),
                text(tr(lang, "dup.minSize")).size(11).style(muted(dark)),
                text_input("1", &d.min_size)
                    .on_input(Msg::DupMinSize)
                    .size(13)
                    .width(Length::Fixed(90.0)),
                btn_primary_scan(dark, lang, d.folder.is_some() && !d.scanning),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            if d.scanning {
                let template = tr(lang, "dup.scanning");
                row![text(fmt(template, &[("n", d.scanned.to_string())]))
                    .size(12)
                    .style(muted(dark))]
            } else {
                row![]
            }
            .padding(iced::Padding {
                top: 8.0,
                ..Default::default()
            }),
        ]
        .spacing(4)
        .width(Length::Fill),
    ));

    if let Some(err) = &d.error {
        col = col.push(error_box(dark, err));
    }
    if let Some(note) = &d.note {
        col = col.push(ok_box(dark, note));
    }

    if let Some(result) = &d.result {
        let selected_bytes: u64 = result
            .groups
            .iter()
            .flat_map(|g| g.files.iter())
            .filter(|f| {
                d.selected
                    .get(&f.path.to_string_lossy().to_string())
                    .copied()
                    .unwrap_or(false)
            })
            .map(|f| f.size)
            .sum();

        let mut results = column![].spacing(16).width(Length::Fill);
        results = results.push(
            row![
                text(format!(
                    "{} {} · {} {} · {} {}",
                    result.stats.duplicate_groups,
                    tr(lang, "dup.groups"),
                    format_bytes(result.stats.wasted_bytes),
                    tr(lang, "dup.wasted"),
                    result.stats.scanned_files,
                    tr(lang, "dup.scanned"),
                ))
                .size(12)
                .style(muted(dark)),
                iced::widget::horizontal_space(),
                btn_danger(
                    dark,
                    &format!(
                        "{} ({})",
                        fmt(
                            tr(lang, "dup.deleteSelected"),
                            &[("n", selected_count.to_string())]
                        ),
                        format_bytes(selected_bytes)
                    ),
                    if !d.scanning && selected_count > 0 {
                        Some(Msg::DupDelete)
                    } else {
                        None
                    },
                ),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );

        if result.groups.is_empty() {
            results = results.push(
                text(tr(lang, "common.noResults"))
                    .size(13)
                    .style(muted(dark)),
            );
        } else {
            results = results.push(text(tr(lang, "dup.keepHint")).size(11).style(muted(dark)));

            let mut groups_col = column![].spacing(12).width(Length::Fill);
            for (index, group) in result.groups.iter().enumerate() {
                let mut files_col = column![].spacing(0).width(Length::Fill);
                for file in &group.files {
                    let path = file.path.to_string_lossy().to_string();
                    let checked = d.selected.get(&path).copied().unwrap_or(false);
                    let path_for_cb = path.clone();
                    files_col = files_col.push(
                        row![
                            checkbox("", checked)
                                .on_toggle(move |v| Msg::DupToggle(path_for_cb.clone(), v))
                                .size(14),
                            text(path.clone())
                                .size(12)
                                .font(font::Font::MONOSPACE)
                                .style(muted(dark))
                                .width(Length::Fill)
                                .wrapping(iced::widget::text::Wrapping::Word),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center)
                        .padding([4, 8]),
                    );
                }
                groups_col = groups_col.push(card(
                    dark,
                    column![
                        row![
                            text(format!("#{}", index + 1))
                                .size(12)
                                .font(font::Font::MONOSPACE),
                            text(format_bytes(group.size)).size(12),
                            text(format!("× {}", group.files.len()))
                                .size(12)
                                .style(muted(dark)),
                            text(format!("{}…", &group.hash[..12.min(group.hash.len())]))
                                .size(12)
                                .font(font::Font::MONOSPACE)
                                .style(muted(dark)),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                        files_col,
                    ]
                    .spacing(8)
                    .width(Length::Fill),
                ));
            }
            results = results.push(scrollable(groups_col).height(Length::Fixed(380.0)));
        }

        col = col.push(card(dark, column![results].spacing(12).width(Length::Fill)));
    }

    col.into()
}

fn btn_primary_scan(dark: bool, lang: crate::i18n::Lang, enabled: bool) -> Element<'static, Msg> {
    let label = tr(lang, "common.scan").to_string();
    crate::views::btn_primary(
        dark,
        &label,
        if enabled { Some(Msg::DupScan) } else { None },
    )
}

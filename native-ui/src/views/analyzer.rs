use iced::widget::{column, container, progress_bar, row, scrollable, text};
use iced::{font, Alignment, Color, Element, Length};

use crate::app::{App, Msg};
use crate::format::{format_bytes, percent};
use crate::i18n::tr;
use crate::palette;
use crate::views::{btn, card, error_box, muted, text_color};

pub fn view(app: &App) -> Element<'_, Msg> {
    let dark = app.dark;
    let lang = app.lang;
    let a = &app.ana;

    let folder_text = a
        .folder
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| tr(lang, "renamer.pickFolderFirst").to_string());

    let mut col = column![].spacing(16).width(Length::Fill);

    col = col.push(card(
        dark,
        row![
            btn(dark, tr(lang, "common.chooseFolder"), Msg::AnaChooseFolder),
            text(folder_text)
                .size(12)
                .font(font::Font::MONOSPACE)
                .style(muted(dark))
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::Word),
            btn(
                dark,
                &if a.busy {
                    tr(lang, "common.loading").to_string()
                } else {
                    tr(lang, "common.analyze").to_string()
                },
                Msg::AnaRun,
            ),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    ));

    if let Some(err) = &a.error {
        col = col.push(error_box(dark, err));
    }

    if let Some(analysis) = &a.analysis {
        let s = palette::scheme(dark);

        let totals = text(format!(
            "{}: {} · {} {} · {} {}",
            tr(lang, "analyzer.totalSize"),
            format_bytes(analysis.total_size),
            analysis.total_files,
            tr(lang, "common.files"),
            analysis.total_dirs,
            tr(lang, "common.folders"),
        ))
        .size(12)
        .style(muted(dark));

        let max_size = analysis.entries.iter().map(|e| e.size).max().unwrap_or(0);
        let mut entries = column![].spacing(0).width(Length::Fill);
        for entry in &analysis.entries {
            let bar_width = percent(entry.size, analysis.total_size);
            let is_largest = entry.size > 0 && entry.size == max_size;
            entries = entries.push(
                row![
                    text(&entry.name)
                        .size(12)
                        .font(font::Font::MONOSPACE)
                        .width(Length::Fixed(240.0))
                        .wrapping(iced::widget::text::Wrapping::Word),
                    container(
                        progress_bar(0.0..=100.0, bar_width)
                            .width(Length::Fill)
                            .height(8.0)
                            .style(move |_| {
                                let s = palette::scheme(dark);
                                progress_bar::Style {
                                    background: s.surface_hover.into(),
                                    bar: if is_largest {
                                        s.accent.into()
                                    } else {
                                        s.border.into()
                                    },
                                    border: iced::Border {
                                        color: Color::TRANSPARENT,
                                        width: 0.0,
                                        radius: 0.0.into(),
                                    },
                                }
                            }),
                    )
                    .width(Length::Fill)
                    .padding(iced::Padding {
                        top: 0.0,
                        right: 12.0,
                        bottom: 0.0,
                        left: 12.0
                    }),
                    text(format_bytes(entry.size))
                        .size(12)
                        .width(Length::Fixed(90.0))
                        .style(text_color(s.text)),
                    text(if entry.files > 0 {
                        entry.files.to_string()
                    } else {
                        String::new()
                    })
                    .size(12)
                    .style(muted(dark))
                    .width(Length::Fixed(70.0)),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .padding([4, 8]),
            );
        }

        let mut largest = column![].spacing(0).width(Length::Fill);
        for file in &analysis.largest {
            let path_str = file.path.to_string_lossy().to_string();
            largest = largest.push(
                row![
                    text(path_str)
                        .size(12)
                        .font(font::Font::MONOSPACE)
                        .style(muted(dark))
                        .width(Length::Fill)
                        .wrapping(iced::widget::text::Wrapping::Word),
                    text(format_bytes(file.size))
                        .size(12)
                        .width(Length::Fixed(90.0)),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .padding([4, 8]),
            );
        }

        col = col.push(card(dark, totals));
        col = col.push(card(
            dark,
            column![
                text(tr(lang, "analyzer.entries")).size(15),
                scrollable(entries).height(Length::Fixed(320.0)),
            ]
            .spacing(12)
            .width(Length::Fill),
        ));
        if !analysis.largest.is_empty() {
            col = col.push(card(
                dark,
                column![text(tr(lang, "analyzer.largest")).size(15), largest,]
                    .spacing(8)
                    .width(Length::Fill),
            ));
        }
    }

    col.into()
}

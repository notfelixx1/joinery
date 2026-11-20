use iced::widget::{column, container, progress_bar, row, scrollable, text};
use iced::{font, Alignment, Color, Element, Length};

use crate::app::analyzer::Msg as AnaMsg;
use crate::app::{App, Msg};
use crate::format::{format_bytes, percent};
use crate::i18n::tr;
use crate::palette;
use crate::views::{btn, card, empty_state, error_box, muted, recent_block, text_color};

/// Alternating row background for long analyzer lists.
fn zebra_row<'a>(idx: usize, el: impl Into<Element<'a, Msg>>) -> Element<'a, Msg> {
    container(el.into())
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: crate::views::zebra(idx).map(Into::into),
            ..Default::default()
        })
        .into()
}

pub fn view(app: &App) -> Element<'_, Msg> {
    let lang = app.lang;
    let a = &app.ana;

    if a.folder.is_none() && a.analysis.is_none() {
        let mut col = column![].spacing(16).width(Length::Fill);
        col = col.push(empty_state(
            crate::icons::pie_chart,
            tr(lang, "tool.analyzer"),
            tr(lang, "empty.analyzer.hint"),
            Some((
                tr(lang, "common.chooseFolder"),
                Msg::Analyzer(AnaMsg::ChooseFolder),
            )),
        ));
        if !app.ana_recent.is_empty() {
            col = col.push(recent_block(
                tr(lang, "empty.recent"),
                &app.ana_recent,
                |p| Msg::Analyzer(AnaMsg::Folder(Some(p))),
            ));
        }
        return col.into();
    }

    let folder_text = a
        .folder
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| tr(lang, "renamer.pickFolderFirst").to_string());

    let mut col = column![].spacing(16).width(Length::Fill);

    col = col.push(card(
        row![
            btn(
                tr(lang, "common.chooseFolder"),
                Msg::Analyzer(AnaMsg::ChooseFolder)
            ),
            text(folder_text)
                .size(12)
                .font(font::Font::MONOSPACE)
                .style(muted())
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::Word),
            btn(
                &if a.busy {
                    tr(lang, "common.loading").to_string()
                } else {
                    tr(lang, "common.analyze").to_string()
                },
                Msg::Analyzer(AnaMsg::Run),
            ),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    ));

    if a.folder.is_some() && a.analysis.is_none() && !a.busy {
        col = col.push(empty_state(
            crate::icons::pie_chart,
            tr(lang, "ana.ready.title"),
            tr(lang, "ana.ready.hint"),
            None,
        ));
    }

    if let Some(err) = &a.error {
        col = col.push(error_box(err));
    }

    if let Some(analysis) = &a.analysis {
        let s = palette::scheme();

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
        .style(muted());

        let max_size = analysis.entries.iter().map(|e| e.size).max().unwrap_or(0);
        let mut entries = column![].spacing(0).width(Length::Fill);
        for (entry_idx, entry) in analysis.entries.iter().enumerate() {
            let bar_width = percent(entry.size, analysis.total_size);
            let is_largest = entry.size > 0 && entry.size == max_size;
            // Label links so their uncounted content is not misread as
            // an empty folder.
            let name = if entry.is_link {
                format!("{} ({})", entry.name, tr(lang, "analyzer.link"))
            } else {
                entry.name.clone()
            };
            entries = entries.push(zebra_row(
                entry_idx,
                row![
                    text(name)
                        .size(12)
                        .font(font::Font::MONOSPACE)
                        .width(Length::Fixed(240.0))
                        .wrapping(iced::widget::text::Wrapping::Word),
                    container(
                        progress_bar(0.0..=100.0, bar_width)
                            .width(Length::Fill)
                            .height(8.0)
                            .style(move |_| {
                                let s = palette::scheme();
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
                    .style(muted())
                    .width(Length::Fixed(70.0)),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .padding([4, 8]),
            ));
        }

        let mut largest = column![].spacing(0).width(Length::Fill);
        for (l_idx, file) in analysis.largest.iter().enumerate() {
            let path_str = file.path.to_string_lossy().to_string();
            largest = largest.push(zebra_row(
                l_idx,
                row![
                    text(path_str)
                        .size(12)
                        .font(font::Font::MONOSPACE)
                        .style(muted())
                        .width(Length::Fill)
                        .wrapping(iced::widget::text::Wrapping::Word),
                    text(format_bytes(file.size))
                        .size(12)
                        .width(Length::Fixed(90.0)),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .padding([4, 8]),
            ));
        }

        col = col.push(card(totals));
        col = col.push(card(
            column![
                text(tr(lang, "analyzer.entries")).size(15),
                scrollable(entries).height(Length::Fixed(320.0)),
            ]
            .spacing(12)
            .width(Length::Fill),
        ));
        if !analysis.largest.is_empty() {
            col = col.push(card(
                column![text(tr(lang, "analyzer.largest")).size(15), largest,]
                    .spacing(8)
                    .width(Length::Fill),
            ));
        }
    }

    col.into()
}

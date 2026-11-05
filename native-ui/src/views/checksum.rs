use iced::widget::{column, row, scrollable, text, text_input};
use iced::{font, Alignment, Element, Length};

use joinery_tools::core::checksum;

use crate::app::{App, ChecksumItem, Msg};
use crate::i18n::tr;
use crate::palette;
use crate::views::{btn, btn_primary, card, empty_state, error_box, muted, text_color};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Alg {
    Sha256,
    Sha1,
    Md5,
}

impl std::fmt::Display for Alg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Alg::Sha256 => "SHA-256",
            Alg::Sha1 => "SHA-1",
            Alg::Md5 => "MD5",
        })
    }
}

impl From<Alg> for checksum::Algorithm {
    fn from(value: Alg) -> Self {
        match value {
            Alg::Sha256 => checksum::Algorithm::Sha256,
            Alg::Sha1 => checksum::Algorithm::Sha1,
            Alg::Md5 => checksum::Algorithm::Md5,
        }
    }
}

impl From<checksum::Algorithm> for Alg {
    fn from(value: checksum::Algorithm) -> Self {
        match value {
            checksum::Algorithm::Sha256 => Alg::Sha256,
            checksum::Algorithm::Sha1 => Alg::Sha1,
            checksum::Algorithm::Md5 => Alg::Md5,
        }
    }
}

const ALGS: [Alg; 3] = [Alg::Sha256, Alg::Sha1, Alg::Md5];

pub fn view(app: &App) -> Element<'_, Msg> {
    let dark = app.dark;
    let lang = app.lang;
    let c = &app.cks;

    if c.paths.is_empty() && c.results.is_none() {
        return empty_state(
            dark,
            crate::icons::shield_check,
            tr(lang, "tool.checksum"),
            tr(lang, "empty.checksum.hint"),
            Some((tr(lang, "common.chooseFiles"), Msg::CksChooseFiles)),
        );
    }

    let s = palette::scheme(dark);

    let names: String = if c.paths.is_empty() {
        tr(lang, "checksum.pickFirst").to_string()
    } else {
        c.paths
            .iter()
            .map(|p| {
                p.to_string_lossy()
                    .rsplit(['\\', '/'])
                    .next()
                    .unwrap_or("")
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join(", ")
    };

    let mut col = column![].spacing(16).width(Length::Fill);

    col = col.push(card(
        dark,
        row![
            btn(dark, tr(lang, "common.chooseFiles"), Msg::CksChooseFiles),
            text(names)
                .size(12)
                .font(font::Font::MONOSPACE)
                .style(muted(dark))
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::Word),
            text(tr(lang, "checksum.algorithm"))
                .size(11)
                .style(muted(dark)),
            iced::widget::pick_list(
                ALGS,
                Some(<checksum::Algorithm as Into<Alg>>::into(c.algorithm)),
                |value| Msg::CksAlgorithm(value.into()),
            )
            .text_size(13),
            btn_primary(
                dark,
                &if c.busy {
                    tr(lang, "common.loading").to_string()
                } else {
                    tr(lang, "common.compute").to_string()
                },
                if !c.paths.is_empty() && !c.busy {
                    Some(Msg::CksRun)
                } else {
                    None
                },
            ),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    ));

    if let Some(err) = &c.error {
        col = col.push(error_box(dark, err));
    }

    if let Some(results) = &c.results {
        let expected = c.expected.trim().to_lowercase();
        let has_match = !expected.is_empty()
            && results.iter().any(|r| {
                r.hash
                    .as_deref()
                    .map(|h| h.to_lowercase() == expected)
                    .unwrap_or(false)
            });

        let mut body = column![].spacing(14).width(Length::Fill);

        body = body.push(
            column![
                text(tr(lang, "checksum.verify"))
                    .size(11)
                    .style(muted(dark)),
                text_input(tr(lang, "checksum.verify.hint"), &c.expected)
                    .on_input(Msg::CksExpected)
                    .size(13)
                    .font(font::Font::MONOSPACE),
                if !expected.is_empty() {
                    if has_match {
                        row![text(tr(lang, "checksum.match"))
                            .size(12)
                            .style(text_color(s.ok))]
                    } else {
                        row![text(tr(lang, "checksum.mismatch"))
                            .size(12)
                            .style(text_color(s.danger))]
                    }
                } else {
                    row![]
                }
                .padding(iced::Padding {
                    top: 4.0,
                    ..Default::default()
                }),
            ]
            .spacing(4)
            .width(Length::Fill),
        );

        let mut rows = column![].spacing(0).width(Length::Fill);
        for item in results {
            rows = rows.push(result_row(dark, lang, item));
        }
        body = body.push(scrollable(rows).height(Length::Fixed(320.0)));

        col = col.push(card(dark, body));
    }

    col.into()
}

fn result_row(dark: bool, lang: crate::i18n::Lang, item: &ChecksumItem) -> Element<'_, Msg> {
    let s = palette::scheme(dark);
    let name = item
        .path
        .to_string_lossy()
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or("")
        .to_string();

    let hash_cell: Element<Msg> = match &item.hash {
        Some(hash) => text(hash)
            .size(12)
            .font(font::Font::MONOSPACE)
            .width(Length::Fill)
            .wrapping(iced::widget::text::Wrapping::Word)
            .into(),
        None => text(format!(
            "{}: {}",
            tr(lang, "common.error"),
            item.error.as_deref().unwrap_or("")
        ))
        .size(12)
        .style(text_color(s.danger))
        .into(),
    };

    let copy_cell: Element<Msg> = if item.hash.is_some() {
        crate::views::btn(
            dark,
            tr(lang, "common.copy"),
            Msg::CksCopy(item.hash.clone().unwrap_or_default()),
        )
    } else {
        text("").size(13).into()
    };

    row![
        text(name)
            .size(12)
            .font(font::Font::MONOSPACE)
            .width(Length::Fixed(180.0))
            .wrapping(iced::widget::text::Wrapping::Word),
        hash_cell,
        copy_cell,
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .padding([4, 8])
    .into()
}

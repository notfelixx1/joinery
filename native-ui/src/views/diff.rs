use iced::widget::{button, column, row, scrollable, text, text_editor};
use iced::{font, Alignment, Element, Length};

use joinery_tools::core::text_diff::DiffKind;

use crate::app::diff::Msg as DiffMsg;
use crate::app::{App, Msg};
use crate::i18n::{fmt, tr};
use crate::palette;
use crate::views::{btn, btn_primary, btn_style, card, error_box, muted, text_color, BtnVariant};

/// Editor pane style: surface background, themed border.
fn editor_style(
    s: &palette::Scheme,
) -> impl Fn(&iced::Theme, iced::widget::text_editor::Status) -> iced::widget::text_editor::Style {
    let style = iced::widget::text_editor::Style {
        background: s.surface.into(),
        border: iced::Border {
            color: s.border,
            width: 1.0,
            radius: 6.0.into(),
        },
        icon: s.text_muted,
        placeholder: s.text_placeholder,
        value: s.text,
        selection: s.accent,
    };
    move |_, _| style
}

pub fn view(app: &App) -> Element<'_, Msg> {
    let lang = app.lang;
    let d = &app.diff;
    let s = palette::scheme();

    let mut col = column![].spacing(16).width(Length::Fill);

    let old_pane = column![
        row![
            text(tr(lang, "dev.oldText")).size(12).style(muted()),
            text(if d.old_text.text().trim().is_empty() {
                tr(lang, "diff.emptyHint")
            } else {
                ""
            })
            .size(12)
            .style(muted())
            .width(Length::Fill),
            btn(tr(lang, "dev.loadFile"), Msg::Diff(DiffMsg::Load(true))),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        text_editor(&d.old_text)
            .height(Length::Fixed(220.0))
            .on_action(|a| Msg::Diff(DiffMsg::Edit(true, a)))
            .size(13)
            .style(editor_style(&s)),
    ]
    .spacing(4)
    .width(Length::Fill);

    let new_pane = column![
        row![
            text(tr(lang, "dev.newText")).size(12).style(muted()),
            text(if d.new_text.text().trim().is_empty() {
                tr(lang, "diff.emptyHint")
            } else {
                ""
            })
            .size(12)
            .style(muted())
            .width(Length::Fill),
            btn(tr(lang, "dev.loadFile"), Msg::Diff(DiffMsg::Load(false))),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        text_editor(&d.new_text)
            .height(Length::Fixed(220.0))
            .on_action(|a| Msg::Diff(DiffMsg::Edit(false, a)))
            .size(13)
            .style(editor_style(&s)),
    ]
    .spacing(4)
    .width(Length::Fill);

    let mut controls = column![].spacing(12).width(Length::Fill);
    controls = controls.push(row![old_pane, new_pane].spacing(12).width(Length::Fill));

    let mut actions = row![
        btn_primary(
            &if d.busy {
                tr(lang, "common.loading").to_string()
            } else {
                tr(lang, "common.compare").to_string()
            },
            if !d.busy {
                Some(Msg::Diff(DiffMsg::Run))
            } else {
                None
            },
        ),
        button(text(tr(lang, "common.clear").to_string()).size(13))
            .padding([8, 16])
            .style(move |_, status| btn_style(BtnVariant::Ghost, status))
            .on_press(Msg::Diff(DiffMsg::Clear)),
        iced::widget::horizontal_space(),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    if let Some(result) = &d.result {
        actions = actions
            .push(
                text(fmt(
                    tr(lang, "dev.added"),
                    &[("n", result.stats.added.to_string())],
                ))
                .size(12)
                .font(font::Font::MONOSPACE)
                .style(text_color(s.ok)),
            )
            .push(
                text(fmt(
                    tr(lang, "dev.removed"),
                    &[("n", result.stats.removed.to_string())],
                ))
                .size(12)
                .font(font::Font::MONOSPACE)
                .style(text_color(s.danger)),
            )
            .push(
                text(fmt(
                    tr(lang, "dev.unchanged"),
                    &[("n", result.stats.unchanged.to_string())],
                ))
                .size(12)
                .font(font::Font::MONOSPACE)
                .style(text_color(s.text_muted)),
            );
    }
    controls = controls.push(actions);

    if let Some(err) = &d.error {
        controls = controls.push(error_box(err));
    }

    col = col.push(card(controls));

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
                    text(row_data.old_line.map(|n| n.to_string()).unwrap_or_default())
                        .size(12)
                        .font(font::Font::MONOSPACE)
                        .style(muted())
                        .width(Length::Fixed(40.0)),
                    text(row_data.new_line.map(|n| n.to_string()).unwrap_or_default())
                        .size(12)
                        .font(font::Font::MONOSPACE)
                        .style(muted())
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
        col = col.push(card(scrollable(rows).height(Length::Fixed(420.0))));
    }

    col.into()
}

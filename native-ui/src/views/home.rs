use iced::widget::{button, column, container, row, svg::Svg, text};
use iced::{Alignment, Color, Element, Length};

use crate::app::{App, Msg, Page};
use crate::i18n::tr;
use crate::icons;
use crate::palette;
use crate::views::{card_style, muted};

struct ToolCard {
    page: Page,
    key: &'static str,
    icon: fn() -> Svg<'static>,
}

const TOOLS: [ToolCard; 6] = [
    ToolCard { page: Page::Files, key: "tool.renamer", icon: icons::tag },
    ToolCard { page: Page::Files, key: "tool.duplicates", icon: icons::copy },
    ToolCard { page: Page::Files, key: "tool.analyzer", icon: icons::pie_chart },
    ToolCard { page: Page::Files, key: "tool.checksum", icon: icons::shield_check },
    ToolCard { page: Page::Developer, key: "tool.jsonyaml", icon: icons::braces },
    ToolCard { page: Page::Developer, key: "tool.diff", icon: icons::columns },
];

pub fn view(app: &App) -> Element<'_, Msg> {
    let dark = app.dark;

    let hero = column![
        text(tr(app.lang, "home.hero.title"))
            .size(26)
            .font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..iced::Font::default()
            })
            .width(Length::Fill)
            .wrapping(iced::widget::text::Wrapping::Word),
        text(tr(app.lang, "home.hero.subtitle"))
            .size(14)
            .style(muted(dark))
            .width(Length::Fill)
            .wrapping(iced::widget::text::Wrapping::Word),
        row![
            badge(dark, tr(app.lang, "home.badges.offline")),
            badge(dark, tr(app.lang, "home.badges.privacy")),
            badge(dark, tr(app.lang, "home.badges.free")),
        ]
        .spacing(8),
    ]
    .spacing(12)
    .width(Length::Fill);

    let mut tools_col = column![
        text(tr(app.lang, "home.tools.title")).size(14),
    ]
    .spacing(16)
    .width(Length::Fill)
    .padding(iced::Padding { top: 36.0, ..Default::default() });

    for pair in TOOLS.chunks(2) {
        let mut r = row![].spacing(16).width(Length::Fill);
        for tool in pair {
            r = r.push(tool_card(app, tool));
        }
        if pair.len() == 1 {
            r = r.push(iced::widget::horizontal_space());
        }
        tools_col = tools_col.push(r);
    }

    column![hero, tools_col].spacing(0).width(Length::Fill).into()
}

fn badge<'a>(dark: bool, label: &'a str) -> Element<'a, Msg> {
    let s = palette::scheme(dark);
    container(text(label).size(12))
        .padding([8, 12])
        .style(move |_| container::Style {
            background: Some(s.surface_active.into()),
            text_color: Some(s.text),
            border: iced::Border {
                color: s.border,
                width: 1.0,
                radius: 20.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn tool_card(app: &App, tool: &ToolCard) -> Element<'static, Msg> {
    let dark = app.dark;
    let s = palette::scheme(dark);
    let title = tr(app.lang, tool.key);
    let desc = tr(app.lang, &format!("{key}.desc", key = tool.key));
    let page = tool.page;
    let icon = (tool.icon)().style(move |_, _| iced::widget::svg::Style {
        color: Some(s.accent),
    });

    let inner = container(
        column![
            row![
                container(icon)
                    .width(Length::Fixed(32.0))
                    .height(Length::Fixed(32.0))
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .style(move |_| container::Style {
                        background: Some(s.surface_active.into()),
                        border: iced::Border {
                            color: Color::TRANSPARENT,
                            width: 0.0,
                            radius: 8.0.into(),
                        },
                        ..Default::default()
                    }),
                column![
                    text(title).size(15),
                    text(desc)
                        .size(12)
                        .style(muted(dark))
                        .wrapping(iced::widget::text::Wrapping::Word),
                ]
                .spacing(4)
                .width(Length::Fill),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        ]
        .spacing(8)
        .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding(16)
    .style(move |_| card_style(dark));

    button(inner)
        .width(Length::Fill)
        .style(move |_, status| {
            let mut st = card_style(dark);
            if let button::Status::Hovered = status {
                st.border.color = s.accent;
            }
            button::Style {
                background: st.background,
                text_color: st.text_color.unwrap_or(s.text),
                border: st.border,
                shadow: Default::default(),
            }
        })
        .on_press(Msg::Nav(page))
        .into()
}

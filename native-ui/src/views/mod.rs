//! Root layout: sidebar + topbar + routed content, plus shared styles.

pub mod analyzer;
pub mod checksum;
pub mod diff;
pub mod duplicates;
pub mod home;
pub mod jsonyaml;
pub mod renamer;
pub mod support;
pub mod system;

use iced::widget::{button, column, container, row, scrollable, svg::Svg, text};
use iced::{Alignment, Color, Element, Length};

use crate::app::{App, DevTool, FileTool, Msg, Page};
use crate::i18n::tr;
use crate::icons;
use crate::palette;

// ---------- styles ----------

pub fn card_style(dark: bool) -> container::Style {
    let s = palette::scheme(dark);
    container::Style {
        background: Some(s.surface.into()),
        text_color: Some(s.text),
        border: iced::Border {
            color: s.border,
            width: 1.0,
            radius: 10.0.into(),
        },
        ..Default::default()
    }
}

fn sidebar_style(dark: bool) -> container::Style {
    let s = palette::scheme(dark);
    container::Style {
        background: Some(s.surface.into()),
        border: iced::Border {
            color: s.border,
            width: if dark { 0.0 } else { 1.0 },
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BtnVariant {
    Plain,
    Primary,
    Danger,
    Ghost,
}

pub fn btn_style(dark: bool, variant: BtnVariant, status: button::Status) -> button::Style {
    let s = palette::scheme(dark);
    let base = match variant {
        BtnVariant::Plain => (Some(s.surface), s.border, s.text),
        BtnVariant::Primary => (Some(s.accent), s.accent, s.accent_text),
        BtnVariant::Danger => (None, s.danger, s.danger),
        BtnVariant::Ghost => (None, Color::TRANSPARENT, s.text_muted),
    };
    let mut style = button::Style {
        background: base.0.map(Into::into),
        text_color: base.2,
        border: iced::Border {
            color: base.1,
            width: if variant == BtnVariant::Ghost {
                0.0
            } else {
                1.0
            },
            radius: 8.0.into(),
        },
        shadow: Default::default(),
    };
    match status {
        button::Status::Hovered => {
            if variant == BtnVariant::Plain {
                style.background = Some(s.surface_hover.into());
            }
            style.border.color = if variant == BtnVariant::Primary {
                s.accent
            } else if variant == BtnVariant::Danger {
                s.danger
            } else if variant == BtnVariant::Ghost {
                Color::TRANSPARENT
            } else {
                s.border
            };
            if variant == BtnVariant::Ghost {
                style.text_color = s.text;
            }
        }
        button::Status::Disabled => {
            style.text_color = s.text_placeholder;
            style.border.color = s.border;
            if variant == BtnVariant::Primary {
                style.background = Some(s.surface_hover.into());
            }
        }
        _ => {}
    }
    style
}

pub fn text_color(color: Color) -> impl Fn(&iced::Theme) -> text::Style + Copy {
    move |_| text::Style { color: Some(color) }
}

pub fn muted(dark: bool) -> impl Fn(&iced::Theme) -> text::Style + Copy {
    let color = palette::scheme(dark).text_muted;
    move |_| text::Style { color: Some(color) }
}

// ---------- shared widgets ----------

pub fn card<'a, M: 'a>(dark: bool, content: impl Into<Element<'a, M>>) -> Element<'a, M> {
    container(content.into())
        .width(Length::Fill)
        .padding(20)
        .style(move |_| card_style(dark))
        .into()
}

pub fn btn<M: Clone + 'static>(dark: bool, label: &str, msg: M) -> Element<'static, M> {
    button(text(label.to_string()).size(13))
        .padding([8, 16])
        .style(move |_, status| btn_style(dark, BtnVariant::Plain, status))
        .on_press(msg)
        .into()
}

pub fn btn_primary<M: Clone + 'static>(
    dark: bool,
    label: &str,
    msg: Option<M>,
) -> Element<'static, M> {
    let mut b = button(text(label.to_string()).size(13))
        .padding([8, 16])
        .style(move |_, status| btn_style(dark, BtnVariant::Primary, status));
    b = match msg {
        Some(m) => b.on_press(m),
        None => b,
    };
    b.into()
}

pub fn btn_danger<M: Clone + 'static>(
    dark: bool,
    label: &str,
    msg: Option<M>,
) -> Element<'static, M> {
    let mut b = button(text(label.to_string()).size(13))
        .padding([8, 16])
        .style(move |_, status| btn_style(dark, BtnVariant::Danger, status));
    b = match msg {
        Some(m) => b.on_press(m),
        None => b,
    };
    b.into()
}

pub fn icon_btn<M: Clone + 'static>(
    dark: bool,
    icon: Element<'static, M>,
    msg: M,
) -> Element<'static, M> {
    button(icon)
        .padding(8)
        .style(move |_, status| btn_style(dark, BtnVariant::Ghost, status))
        .on_press(msg)
        .into()
}

/// Sidebar nav icon: muted when idle, accent when the page is active.
fn nav_icon(dark: bool, active: bool, icon: Svg<'static>) -> Element<'static, Msg> {
    let s = palette::scheme(dark);
    icon.style(move |_, _| iced::widget::svg::Style {
        color: Some(if active { s.accent } else { s.text_muted }),
    })
    .into()
}

/// Topbar icon: muted when idle, full text color on hover.
fn topbar_icon(dark: bool, icon: Svg<'static>) -> Element<'static, Msg> {
    let s = palette::scheme(dark);
    icon.style(move |_, status| iced::widget::svg::Style {
        color: Some(if matches!(status, iced::widget::svg::Status::Hovered) {
            s.text
        } else {
            s.text_muted
        }),
    })
    .into()
}

pub fn error_box<M: 'static>(dark: bool, message: &str) -> Element<'static, M> {
    let s = palette::scheme(dark);
    container(
        text(message.to_string())
            .size(13)
            .style(text_color(s.danger))
            .wrapping(iced::widget::text::Wrapping::Word),
    )
    .width(Length::Fill)
    .padding(10)
    .style(move |_| container::Style {
        background: Some(
            Color {
                r: s.danger.r,
                g: s.danger.g,
                b: s.danger.b,
                a: 0.06,
            }
            .into(),
        ),
        text_color: Some(s.danger),
        border: iced::Border {
            color: Color {
                r: s.danger.r,
                g: s.danger.g,
                b: s.danger.b,
                a: 0.15,
            },
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    })
    .into()
}

pub fn ok_box<M: 'static>(dark: bool, message: &str) -> Element<'static, M> {
    let s = palette::scheme(dark);
    container(
        text(message.to_string())
            .size(13)
            .style(text_color(s.ok))
            .wrapping(iced::widget::text::Wrapping::Word),
    )
    .width(Length::Fill)
    .padding(10)
    .style(move |_| container::Style {
        background: Some(
            Color {
                r: s.ok.r,
                g: s.ok.g,
                b: s.ok.b,
                a: 0.06,
            }
            .into(),
        ),
        text_color: Some(s.ok),
        border: iced::Border {
            color: Color {
                r: s.ok.r,
                g: s.ok.g,
                b: s.ok.b,
                a: 0.15,
            },
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    })
    .into()
}

pub fn field<'a>(dark: bool, label: &str, input: impl Into<Element<'a, Msg>>) -> Element<'a, Msg> {
    column![
        text(label.to_string()).size(12).style(muted(dark)),
        input.into(),
    ]
    .spacing(4)
    .width(Length::Fill)
    .into()
}

pub fn divider<'a, M: 'a>(dark: bool) -> Element<'a, M> {
    let s = palette::scheme(dark);
    container(iced::widget::horizontal_rule(1.0))
        .style(move |_| container::Style {
            background: Some(s.divider.into()),
            ..Default::default()
        })
        .height(Length::Fixed(1.0))
        .width(Length::Fill)
        .into()
}

// ---------- layout ----------

fn logo(dark: bool) -> Element<'static, Msg> {
    let s = palette::scheme(dark);
    container(text("J").size(17).style(move |_| text::Style {
        color: Some(if dark { s.background } else { Color::WHITE }),
    }))
    .width(32)
    .height(32)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .style(move |_| container::Style {
        background: Some(s.accent.into()),
        border: iced::Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    })
    .into()
}

fn nav_button(
    dark: bool,
    icon: Element<'static, Msg>,
    label: &str,
    active: bool,
    msg: Msg,
) -> Element<'static, Msg> {
    let s = palette::scheme(dark);
    let label = label.to_string();
    button(
        row![icon, text(label).size(13)]
            .spacing(10)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([9, 12])
    .style(move |_, status| {
        let mut st = button::Style {
            background: Some(if active {
                s.surface_active.into()
            } else {
                Color::TRANSPARENT.into()
            }),
            text_color: if active { s.accent } else { s.text_muted },
            border: iced::Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 8.0.into(),
            },
            shadow: Default::default(),
        };
        if active {
            st.border = iced::Border {
                color: s.accent,
                width: 0.0,
                radius: 8.0.into(),
            };
        }
        if let button::Status::Hovered = status {
            if !active {
                st.background = Some(s.surface_hover.into());
                st.text_color = s.text;
            }
        }
        st
    })
    .on_press(msg)
    .into()
}

fn sidebar(app: &App) -> Element<'_, Msg> {
    let dark = app.dark;
    let items = [
        (Page::Home, "nav.home", icons::home as fn() -> Svg<'static>),
        (
            Page::Files,
            "nav.files",
            icons::folder as fn() -> Svg<'static>,
        ),
        (
            Page::Developer,
            "nav.developer",
            icons::code as fn() -> Svg<'static>,
        ),
        (
            Page::System,
            "nav.system",
            icons::monitor as fn() -> Svg<'static>,
        ),
        (
            Page::Support,
            "nav.support",
            icons::heart as fn() -> Svg<'static>,
        ),
    ];

    let mut col = column![]
        .spacing(4)
        .width(Length::Fill)
        .height(Length::Fill);
    col = col.push(
        row![
            logo(dark),
            column![
                text("Joinery").size(16).font(iced::Font {
                    weight: iced::font::Weight::Semibold,
                    ..iced::Font::default()
                }),
                text(tr(app.lang, "app.tagline"))
                    .size(12)
                    .style(muted(dark)),
            ]
            .spacing(2)
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .padding(iced::Padding {
            top: 4.0,
            right: 8.0,
            bottom: 24.0,
            left: 8.0,
        }),
    );

    for (page, key, icon) in items {
        col = col.push(nav_button(
            dark,
            nav_icon(dark, app.page == page, icon()),
            tr(app.lang, key),
            app.page == page,
            Msg::Nav(page),
        ));
    }

    col = col.push(iced::widget::vertical_space());
    col = col.push(divider(dark));
    col = col.push(
        column![
            text(tr(app.lang, "footer.license"))
                .size(12)
                .style(muted(dark)),
            text("v0.1.0").size(12).style(muted(dark)),
        ]
        .spacing(2)
        .padding(iced::Padding {
            top: 12.0,
            right: 12.0,
            bottom: 4.0,
            left: 12.0,
        }),
    );

    container(col)
        .width(Length::Fixed(252.0))
        .height(Length::Fill)
        .padding([20, 16])
        .style(move |_| sidebar_style(dark))
        .into()
}

fn topbar(app: &App) -> Element<'_, Msg> {
    let title_key = match app.page {
        Page::Home => "nav.home",
        Page::Files => "nav.files",
        Page::Developer => "nav.developer",
        Page::System => "nav.system",
        Page::Support => "nav.support",
    };
    row![
        text(tr(app.lang, title_key)).size(20),
        iced::widget::horizontal_space(),
        icon_btn(
            app.dark,
            topbar_icon(app.dark, icons::globe()),
            Msg::ToggleLang
        ),
        icon_btn(
            app.dark,
            topbar_icon(
                app.dark,
                if app.dark {
                    icons::sun()
                } else {
                    icons::moon()
                }
            ),
            Msg::ToggleTheme
        ),
    ]
    .spacing(4)
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .padding(iced::Padding {
        top: 16.0,
        right: 28.0,
        bottom: 0.0,
        left: 28.0,
    })
    .into()
}

const FILE_TABS: [(FileTool, &str); 4] = [
    (FileTool::Renamer, "tool.renamer"),
    (FileTool::Duplicates, "tool.duplicates"),
    (FileTool::Analyzer, "tool.analyzer"),
    (FileTool::Checksum, "tool.checksum"),
];

const DEV_TABS: [(DevTool, &str); 2] = [
    (DevTool::JsonYaml, "dev.jsonyaml"),
    (DevTool::Diff, "dev.diff"),
];

pub fn file_tabs(app: &App) -> Element<'_, Msg> {
    tab_row(app, &FILE_TABS, |tool| app.file_tool == tool, Msg::FileTool)
}

pub fn dev_tabs(app: &App) -> Element<'_, Msg> {
    tab_row(app, &DEV_TABS, |tool| app.dev_tool == tool, Msg::DevTool)
}

fn tab_row<'a, T: Copy + 'static>(
    app: &'a App,
    tabs: &'static [(T, &'static str)],
    is_active: impl Fn(T) -> bool,
    msg: impl Fn(T) -> Msg + Copy + 'static,
) -> Element<'a, Msg> {
    let s = palette::scheme(app.dark);
    let mut r = row![].spacing(4);
    for (tool, key) in tabs {
        let active = is_active(*tool);
        let label = tr(app.lang, key).to_string();
        let m = msg(*tool);
        let tab_btn = button(text(label).size(13))
            .padding([8, 4])
            .style(move |_, status| {
                let mut st = button::Style {
                    background: Some(Color::TRANSPARENT.into()),
                    text_color: if active { s.text } else { s.text_muted },
                    border: iced::Border {
                        color: Color::TRANSPARENT,
                        width: 0.0,
                        radius: 0.0.into(),
                    },
                    shadow: Default::default(),
                };
                if let button::Status::Hovered = status {
                    if !active {
                        st.text_color = s.text;
                    }
                }
                st
            })
            .on_press(m);
        let underline = container(iced::widget::Space::with_height(Length::Fixed(2.0)))
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(if active {
                    s.accent.into()
                } else {
                    Color::TRANSPARENT.into()
                }),
                ..Default::default()
            });
        r = r.push(column![tab_btn, underline].spacing(4).width(Length::Shrink));
    }
    container(r)
        .width(Length::Fill)
        .padding(iced::Padding {
            bottom: 8.0,
            ..Default::default()
        })
        .style(move |_| container::Style {
            border: iced::Border {
                color: s.border,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

pub fn view(app: &App) -> Element<'_, Msg> {
    let content: Element<Msg> = match app.page {
        Page::Home => home::view(app),
        Page::Files => column![
            file_tabs(app),
            match app.file_tool {
                FileTool::Renamer => renamer::view(app),
                FileTool::Duplicates => duplicates::view(app),
                FileTool::Analyzer => analyzer::view(app),
                FileTool::Checksum => checksum::view(app),
            }
        ]
        .spacing(16)
        .width(Length::Fill)
        .into(),
        Page::Developer => column![
            dev_tabs(app),
            match app.dev_tool {
                DevTool::JsonYaml => jsonyaml::view(app),
                DevTool::Diff => diff::view(app),
            }
        ]
        .spacing(16)
        .width(Length::Fill)
        .into(),
        Page::System => system::view(app),
        Page::Support => support::view(app),
    };

    let main = column![
        topbar(app),
        scrollable(
            container(content)
                .padding(iced::Padding {
                    top: 16.0,
                    right: 28.0,
                    bottom: 40.0,
                    left: 28.0
                })
                .width(Length::Fill)
        )
        .width(Length::Fill)
        .height(Length::Fill),
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    row![sidebar(app), main]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

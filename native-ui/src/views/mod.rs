//! Root layout: sidebar + topbar + routed content, plus shared styles.

pub mod analyzer;
pub mod checksum;
pub mod diff;
pub mod duplicates;
pub mod history;
pub mod home;
pub mod jsonyaml;
pub mod renamer;
pub mod support;
pub mod system;

use std::path::PathBuf;

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
            if variant == BtnVariant::Plain || variant == BtnVariant::Ghost {
                style.background = Some(s.surface_hover.into());
            }
            if variant == BtnVariant::Danger {
                style.background = Some(s.danger_tint(0.08).into());
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
        button::Status::Pressed => {
            // Tactile feedback: the button sinks one surface step.
            match variant {
                BtnVariant::Plain | BtnVariant::Ghost => {
                    style.background = Some(s.surface_active.into());
                    style.text_color = s.text;
                }
                BtnVariant::Primary => {
                    style.background = Some(dim(s.accent, 0.85).into());
                    style.text_color = s.accent_text;
                }
                BtnVariant::Danger => {
                    style.background = Some(s.danger_tint(0.16).into());
                    style.border.color = s.danger;
                }
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

/// Slightly darken a fill color for pressed feedback.
fn dim(color: Color, factor: f32) -> Color {
    Color {
        r: color.r * factor,
        g: color.g * factor,
        b: color.b * factor,
        a: color.a,
    }
}

/// Centered "nothing here yet" card: icon, one line, one action.
pub fn empty_state<'a, M: Clone + 'static>(
    dark: bool,
    icon: fn() -> Svg<'static>,
    title: &str,
    hint: &str,
    action: Option<(&'a str, M)>,
) -> Element<'a, M> {
    let s = palette::scheme(dark);
    let icon_widget = icon().style(move |_, _| iced::widget::svg::Style {
        color: Some(s.text_muted),
    });
    let mut col = column![]
        .spacing(10)
        .align_x(Alignment::Center)
        .max_width(520)
        .width(Length::Fill);
    col = col.push(
        container(icon_widget)
            .width(Length::Fixed(44.0))
            .height(Length::Fixed(44.0))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(move |_| container::Style {
                background: Some(s.surface_hover.into()),
                border: iced::Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 10.0.into(),
                },
                ..Default::default()
            }),
    );
    col = col.push(text(title.to_string()).size(15));
    col = col.push(
        text(hint.to_string())
            .size(12)
            .style(muted(dark))
            .wrapping(iced::widget::text::Wrapping::Word),
    );
    if let Some((label, msg)) = action {
        col = col.push(btn_primary(dark, label, Some(msg)));
    }
    container(col)
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .padding(48)
        .style(move |_| card_style(dark))
        .into()
}

/// Quiet list of recently used folders; clicking one replays the
/// folder-pick message without opening the dialog.
pub fn recent_block<'a, M: Clone + 'static>(
    dark: bool,
    label: &str,
    items: &[PathBuf],
    on_pick: impl Fn(String) -> M + Copy + 'a,
) -> Element<'a, M> {
    let mut col = column![]
        .spacing(4)
        .max_width(520)
        .width(Length::Fill)
        .align_x(Alignment::Center);
    col = col.push(text(label.to_string()).size(12).style(muted(dark)));
    for item in items {
        let path = item.to_string_lossy().to_string();
        col = col.push(
            button(
                text(path.clone())
                    .size(12)
                    .font(iced::font::Font::MONOSPACE)
                    .style(move |_| text::Style {
                        color: Some(palette::scheme(dark).text_muted),
                    }),
            )
            .padding([6, 10])
            .width(Length::Fill)
            .style(move |_, status| {
                let mut st = button::Style {
                    background: Some(Color::TRANSPARENT.into()),
                    text_color: palette::scheme(dark).text_muted,
                    border: iced::Border {
                        color: Color::TRANSPARENT,
                        width: 0.0,
                        radius: 8.0.into(),
                    },
                    shadow: Default::default(),
                };
                if let button::Status::Hovered = status {
                    st.background = Some(palette::scheme(dark).surface_hover.into());
                    st.text_color = palette::scheme(dark).text;
                }
                st
            })
            .on_press(on_pick(path)),
        );
    }
    container(col)
        .width(Length::Fill)
        .align_x(Alignment::Center)
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
            Page::History,
            "nav.history",
            icons::clock as fn() -> Svg<'static>,
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
        Page::History => "nav.history",
        Page::System => "nav.system",
        Page::Support => "nav.support",
    };
    row![
        text(tr(app.lang, title_key)).size(20),
        iced::widget::horizontal_space(),
        iced::widget::tooltip(
            icon_btn(
                app.dark,
                topbar_icon(app.dark, icons::globe()),
                Msg::ToggleLang
            ),
            text(tr(app.lang, "lang.toggle")).size(12),
            iced::widget::tooltip::Position::Bottom,
        ),
        iced::widget::tooltip(
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
            text(tr(app.lang, "theme.toggle")).size(12),
            iced::widget::tooltip::Position::Bottom,
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
        let tab_btn = button(
            container(text(label).size(13))
                .width(Length::Fill)
                .align_x(Alignment::Center),
        )
        .width(Length::Fill)
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
        r = r.push(column![tab_btn, underline].spacing(4).width(Length::Fill));
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

/// True when the current screen is an empty state: those render without
/// the scroll (a scrollable gives its content unbounded height, where
/// `Fill` collapses) and center vertically in the window instead.
fn is_empty_view(app: &App) -> bool {
    match app.page {
        Page::Files => match app.file_tool {
            FileTool::Renamer => app.ren.folder.is_none(),
            FileTool::Duplicates => {
                app.dup.folder.is_none() && app.dup.result.is_none() && !app.dup.scanning
            }
            FileTool::Analyzer => app.ana.folder.is_none() && app.ana.analysis.is_none(),
            FileTool::Checksum => app.cks.paths.is_empty() && app.cks.results.is_none(),
        },
        Page::History => app.history.is_empty(),
        _ => false,
    }
}

pub fn view(app: &App) -> Element<'_, Msg> {
    let empty = is_empty_view(app);

    // Tabs are built per branch: `Element` is not `Clone`, and both
    // branches need their own instance.
    let content: Element<Msg> = match app.page {
        Page::Home => home::view(app),
        Page::Files => match app.file_tool {
            FileTool::Renamer => renamer::view(app),
            FileTool::Duplicates => duplicates::view(app),
            FileTool::Analyzer => analyzer::view(app),
            FileTool::Checksum => checksum::view(app),
        },
        Page::Developer => match app.dev_tool {
            DevTool::JsonYaml => jsonyaml::view(app),
            DevTool::Diff => diff::view(app),
        },
        Page::History => history::view(app),
        Page::System => system::view(app),
        Page::Support => support::view(app),
    };

    let body: Element<Msg> = if empty {
        // Tabs stay on top, the empty state centers in the remaining
        // height — no scroll, nothing to scroll to anyway.
        let centered = container(container(content).padding(iced::Padding {
            top: 16.0,
            right: 28.0,
            bottom: 16.0,
            left: 28.0,
        }))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(Alignment::Center);
        match app.page {
            Page::Files => column![file_tabs(app), centered]
                .spacing(16)
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            Page::Developer => column![dev_tabs(app), centered]
                .spacing(16)
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            _ => centered.into(),
        }
    } else {
        let with_tabs: Element<Msg> = match app.page {
            Page::Files => column![file_tabs(app), content]
                .spacing(16)
                .width(Length::Fill)
                .into(),
            Page::Developer => column![dev_tabs(app), content]
                .spacing(16)
                .width(Length::Fill)
                .into(),
            _ => content,
        };
        column![scrollable(
            container(with_tabs)
                .padding(iced::Padding {
                    top: 16.0,
                    right: 28.0,
                    bottom: 40.0,
                    left: 28.0,
                })
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill),]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    };

    row![
        sidebar(app),
        column![topbar(app), body]
            .width(Length::Fill)
            .height(Length::Fill)
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

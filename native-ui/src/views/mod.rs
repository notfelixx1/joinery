//! Root layout: sidebar + topbar + routed content, plus shared styles.

pub mod analyzer;
pub mod checksum;
pub mod diff;
pub mod duplicates;
pub mod history;
pub mod home;
pub mod jsonyaml;
pub mod renamer;
pub mod settings;
pub mod support;
pub mod system;

use std::path::PathBuf;

use iced::widget::{button, column, container, row, scrollable, svg::Svg, text};
use iced::{Alignment, Color, Element, Length};

use crate::app::NavReveal;
use crate::app::{App, DevTool, FileTool, Msg, Page};
use crate::i18n::{tr, Lang};
use crate::icons;
use crate::palette;

// ---------- styles ----------

pub fn card_style() -> container::Style {
    let s = palette::scheme();
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

fn sidebar_style() -> container::Style {
    let s = palette::scheme();
    container::Style {
        background: Some(s.panel.into()),
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

pub fn btn_style(variant: BtnVariant, status: button::Status) -> button::Style {
    let s = palette::scheme();
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

/// Themed pick-list field: surface background, themed border, accent
/// handle on hover. Replaces the default gray style that clashed with
/// the dark theme.
pub fn pick_field_style(
    _theme: &iced::Theme,
    status: iced::widget::pick_list::Status,
) -> iced::widget::pick_list::Style {
    let s = palette::scheme();
    let (bg, border_c, handle_c) = match status {
        iced::widget::pick_list::Status::Hovered | iced::widget::pick_list::Status::Opened => {
            (s.surface_hover, s.accent, s.accent)
        }
        _ => (s.surface, s.border, s.text_muted),
    };
    iced::widget::pick_list::Style {
        text_color: s.text,
        placeholder_color: s.text_muted,
        handle_color: handle_c,
        background: bg.into(),
        border: iced::Border {
            color: border_c,
            width: 1.0,
            radius: 6.0.into(),
        },
    }
}

/// Themed dropdown menu for the pick list.
pub fn pick_menu_style(_theme: &iced::Theme) -> iced::overlay::menu::Style {
    let s = palette::scheme();
    let selected_bg = s.surface_active;
    iced::overlay::menu::Style {
        background: s.surface.into(),
        border: iced::Border {
            color: s.border,
            width: 1.0,
            radius: 8.0.into(),
        },
        text_color: s.text,
        selected_text_color: s.accent,
        selected_background: selected_bg.into(),
    }
}

/// Themed checkbox: accent fill with readable check when checked,
/// surface with border when not.
pub fn checkbox_style(
    _theme: &iced::Theme,
    status: iced::widget::checkbox::Status,
) -> iced::widget::checkbox::Style {
    let s = palette::scheme();
    let is_checked = match status {
        iced::widget::checkbox::Status::Active { is_checked }
        | iced::widget::checkbox::Status::Hovered { is_checked }
        | iced::widget::checkbox::Status::Disabled { is_checked } => is_checked,
    };
    let hovered = matches!(status, iced::widget::checkbox::Status::Hovered { .. });
    if is_checked {
        iced::widget::checkbox::Style {
            background: s.accent.into(),
            icon_color: s.accent_text,
            border: iced::Border {
                color: if hovered { s.text } else { s.accent },
                width: 1.0,
                radius: 4.0.into(),
            },
            text_color: None,
        }
    } else {
        iced::widget::checkbox::Style {
            background: if hovered { s.surface_hover } else { s.surface }.into(),
            icon_color: Color::TRANSPARENT,
            border: iced::Border {
                color: if hovered { s.accent } else { s.border },
                width: 1.0,
                radius: 4.0.into(),
            },
            text_color: None,
        }
    }
}

/// Alternating row tint for long lists: odd rows get a faint mix of
/// the surface toward the text color.
pub fn zebra(idx: usize) -> Option<iced::Color> {
    let s = palette::scheme();
    if idx % 2 == 1 {
        Some(palette::mix(s.surface, s.text, 0.04))
    } else {
        None
    }
}

pub fn muted() -> impl Fn(&iced::Theme) -> text::Style + Copy {
    let color = palette::scheme().text_muted;
    move |_| text::Style { color: Some(color) }
}

// ---------- shared widgets ----------

pub fn card<'a, M: 'a>(content: impl Into<Element<'a, M>>) -> Element<'a, M> {
    container(content.into())
        .width(Length::Fill)
        .padding(20)
        .style(move |_| card_style())
        .into()
}

pub fn btn<M: Clone + 'static>(label: &str, msg: M) -> Element<'static, M> {
    button(text(label.to_string()).size(13))
        .padding([8, 16])
        .style(move |_, status| btn_style(BtnVariant::Plain, status))
        .on_press(msg)
        .into()
}

pub fn btn_primary<M: Clone + 'static>(label: &str, msg: Option<M>) -> Element<'static, M> {
    let mut b = button(text(label.to_string()).size(13))
        .padding([8, 16])
        .style(move |_, status| btn_style(BtnVariant::Primary, status));
    b = match msg {
        Some(m) => b.on_press(m),
        None => b,
    };
    b.into()
}

pub fn btn_danger<M: Clone + 'static>(label: &str, msg: Option<M>) -> Element<'static, M> {
    let mut b = button(text(label.to_string()).size(13))
        .padding([8, 16])
        .style(move |_, status| btn_style(BtnVariant::Danger, status));
    b = match msg {
        Some(m) => b.on_press(m),
        None => b,
    };
    b.into()
}

/// Sidebar nav icon: muted when idle, accent when the page is active.
fn nav_icon(active: bool, icon: Svg<'static>) -> Element<'static, Msg> {
    let s = palette::scheme();
    icon.style(move |_, _| iced::widget::svg::Style {
        color: Some(if active { s.accent } else { s.text_muted }),
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
    icon: fn() -> Svg<'static>,
    title: &str,
    hint: &str,
    action: Option<(&'a str, M)>,
) -> Element<'a, M> {
    let s = palette::scheme();
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
            .style(muted())
            .wrapping(iced::widget::text::Wrapping::Word),
    );
    if let Some((label, msg)) = action {
        col = col.push(btn_primary(label, Some(msg)));
    }
    container(col)
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .padding(48)
        .style(move |_| card_style())
        .into()
}

/// Quiet list of recently used folders; clicking one replays the
/// folder-pick message without opening the dialog.
pub fn recent_block<'a, M: Clone + 'static>(
    label: &str,
    items: &[PathBuf],
    on_pick: impl Fn(String) -> M + Copy + 'a,
) -> Element<'a, M> {
    let mut col = column![]
        .spacing(4)
        .max_width(520)
        .width(Length::Fill)
        .align_x(Alignment::Center);
    col = col.push(text(label.to_string()).size(12).style(muted()));
    for item in items {
        let path = item.to_string_lossy().to_string();
        col = col.push(
            button(
                text(path.clone())
                    .size(12)
                    .font(iced::font::Font::MONOSPACE)
                    .style(move |_| text::Style {
                        color: Some(palette::scheme().text_muted),
                    }),
            )
            .padding([6, 10])
            .width(Length::Fill)
            .style(move |_, status| {
                let mut st = button::Style {
                    background: Some(Color::TRANSPARENT.into()),
                    text_color: palette::scheme().text_muted,
                    border: iced::Border {
                        color: Color::TRANSPARENT,
                        width: 0.0,
                        radius: 8.0.into(),
                    },
                    shadow: Default::default(),
                };
                if let button::Status::Hovered = status {
                    st.background = Some(palette::scheme().surface_hover.into());
                    st.text_color = palette::scheme().text;
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

pub fn error_box<M: 'static>(message: &str) -> Element<'static, M> {
    let s = palette::scheme();
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

pub fn ok_box<M: 'static>(message: &str) -> Element<'static, M> {
    let s = palette::scheme();
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

pub fn field<'a>(label: &str, input: impl Into<Element<'a, Msg>>) -> Element<'a, Msg> {
    column![
        text(label.to_string()).size(12).style(muted()),
        input.into(),
    ]
    .spacing(4)
    .width(Length::Fill)
    .into()
}

pub fn divider<'a, M: 'a>() -> Element<'a, M> {
    let s = palette::scheme();
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

fn logo() -> Element<'static, Msg> {
    use iced::Color;

    // Brand mark: fixed indigo tile — does not follow the user accent.
    let brand = palette::BRAND_INDIGO;
    container(
        icons::logo_mark()
            .width(Length::Fixed(20.0))
            .height(Length::Fixed(20.0))
            .style(move |_, _| iced::widget::svg::Style {
                color: Some(Color::WHITE),
            }),
    )
    .width(Length::Fixed(32.0))
    .height(Length::Fixed(32.0))
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .style(move |_| container::Style {
        background: Some(brand.into()),
        border: iced::Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}
/// Shared style for sidebar buttons: active items sit on the active
/// surface in the accent color, idle ones are muted and fill on hover.
fn nav_btn_style(active: bool) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    let s = palette::scheme();
    move |_, status| {
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
    }
}

fn nav_button(
    icon: Element<'static, Msg>,
    label: &str,
    active: bool,
    msg: Msg,
) -> Element<'static, Msg> {
    let label = label.to_string();
    button(
        row![icon, text(label).size(13)]
            .spacing(10)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([9, 12])
    .style(nav_btn_style(active))
    .on_press(msg)
    .into()
}

/// A sidebar section header: icon, label and a chevron that points
/// into the group when it is collapsed and down when expanded.
fn nav_header(
    icon: Element<'static, Msg>,
    label: String,
    expanded: bool,
    active: bool,
    msg: Msg,
) -> Element<'static, Msg> {
    let s = palette::scheme();
    let chevron = if expanded {
        icons::chevron_down()
    } else {
        icons::chevron_right()
    };
    button(
        row![
            icon,
            text(label).size(13),
            iced::widget::horizontal_space(),
            chevron
                .width(Length::Fixed(14.0))
                .height(Length::Fixed(14.0))
                .style(move |_, _| iced::widget::svg::Style {
                    color: Some(s.text_muted),
                }),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([9, 12])
    .style(nav_btn_style(active))
    .on_press(msg)
    .into()
}

/// An indented sub-item inside an expanded section.
fn nav_sub_item(label: &str, active: bool, msg: Msg) -> Element<'static, Msg> {
    let label = label.to_string();
    iced::widget::container(
        button(text(label).size(13))
            .width(Length::Fill)
            .padding([7, 10])
            .style(nav_btn_style(active))
            .on_press(msg),
    )
    .padding(iced::Padding {
        left: 20.0,
        right: 2.0,
        ..iced::Padding::default()
    })
    .width(Length::Fill)
    .into()
}

/// Height of a section's sub-item stack in pixels while its reveal is
/// in flight: the estimate scales with the animation progress `t`,
/// opening or closing. Used only during the slide — at rest the stack
/// takes its natural content height, so nothing is ever clipped and no
/// dead gap appears below the last item.
fn section_stack_height(open: bool, reveal: Option<&NavReveal>, page: Page, full: f32) -> f32 {
    match reveal {
        Some(r) if r.page == page => {
            let k = if r.opening { r.t } else { 1.0 - r.t };
            k * full
        }
        _ if open => full,
        _ => 0.0,
    }
}

/// Subtle card behind an expanded section's sub-items — the group reads
/// as one block instead of items floating over empty space.
fn nav_group_block() -> container::Style {
    let s = palette::scheme();
    container::Style {
        background: Some(s.surface.into()),
        border: iced::Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 8.0.into(),
        },
        shadow: Default::default(),
        ..container::Style::default()
    }
}

/// A collapsible sidebar section. The header always renders; the
/// sub-items sit on a tinted block and slide out of (or back into) a
/// clipped container during the reveal, which `AnimTick` advances.
fn nav_section(
    app: &App,
    page: Page,
    key: &str,
    icon: fn() -> Svg<'static>,
    subs: &[(&str, Msg, bool)],
) -> Element<'static, Msg> {
    let open = app.nav_open == Some(page);
    let active = app.page == page;
    let reveal = app.nav_reveal.as_ref().filter(|r| r.page == page);

    // Estimated geometry of the sub-item stack, measured against the
    // rendered sidebar: one item is ~40 px (a two-line label adds
    // ~20 px — "Массовое переименование" wraps in Russian). The estimate
    // only drives the moving clip during the slide; at rest the stack
    // keeps its natural height.
    const ITEM_H: f32 = 40.0;
    const WRAP_SLACK: f32 = 20.0;
    const SPACING: f32 = 4.0;
    let full =
        subs.len() as f32 * ITEM_H + subs.len().saturating_sub(1) as f32 * SPACING + WRAP_SLACK;

    let mut section = column![nav_header(
        nav_icon(active, icon()),
        tr(app.lang, key).to_string(),
        open,
        active,
        Msg::ToggleNav(page),
    )]
    .spacing(SPACING)
    .width(Length::Fill);

    if open || reveal.is_some() {
        let mut sub_col = column![].spacing(SPACING);
        for (label, msg, sub_active) in subs {
            sub_col = sub_col.push(nav_sub_item(label, *sub_active, msg.clone()));
        }
        let stack: Element<'static, Msg> = match reveal {
            Some(r) => iced::widget::container(sub_col)
                .width(Length::Fill)
                .height(Length::Fixed(section_stack_height(
                    true,
                    Some(r),
                    page,
                    full,
                )))
                .clip(true)
                .into(),
            None => iced::widget::container(sub_col).width(Length::Fill).into(),
        };
        section = section.push(
            iced::widget::container(stack)
                .padding(iced::Padding {
                    top: 3.0,
                    right: 6.0,
                    bottom: 3.0,
                    left: 6.0,
                })
                .width(Length::Fill)
                .style(move |_| nav_group_block()),
        );
    }

    section.into()
}

fn sidebar(app: &App) -> Element<'_, Msg> {
    let mut col = column![]
        .spacing(4)
        .width(Length::Fill)
        .height(Length::Fill);
    col = col.push(
        row![
            logo(),
            column![
                text("Joinery").size(16).font(iced::Font {
                    weight: iced::font::Weight::Semibold,
                    ..iced::Font::default()
                }),
                text(tr(app.lang, "app.tagline")).size(12).style(muted()),
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

    col = col.push(nav_button(
        nav_icon(app.page == Page::Home, icons::home()),
        tr(app.lang, "nav.home"),
        app.page == Page::Home,
        Msg::Nav(Page::Home),
    ));

    // "Files" and "Developer" are expandable groups: the header toggles
    // the slide-out, the sub-items jump straight to a tool.
    let files_subs: Vec<(&str, Msg, bool)> = FILE_TABS
        .iter()
        .map(|(tool, key)| {
            (
                tr(app.lang, key),
                Msg::FileTool(*tool),
                app.page == Page::Files && app.file_tool == *tool,
            )
        })
        .collect();
    col = col.push(nav_section(
        app,
        Page::Files,
        "nav.files",
        icons::folder,
        &files_subs,
    ));

    let dev_subs: Vec<(&str, Msg, bool)> = DEV_TABS
        .iter()
        .map(|(tool, key)| {
            (
                tr(app.lang, key),
                Msg::DevTool(*tool),
                app.page == Page::Developer && app.dev_tool == *tool,
            )
        })
        .collect();
    col = col.push(nav_section(
        app,
        Page::Developer,
        "nav.developer",
        icons::code,
        &dev_subs,
    ));

    for (page, key, icon) in [
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
    ] {
        col = col.push(nav_button(
            nav_icon(app.page == page, icon()),
            tr(app.lang, key),
            app.page == page,
            Msg::Nav(page),
        ));
    }

    col = col.push(iced::widget::vertical_space());
    col = col.push(divider());
    col = col.push(nav_button(
        nav_icon(app.page == Page::Settings, icons::settings()),
        tr(app.lang, "nav.settings"),
        app.page == Page::Settings,
        Msg::Nav(Page::Settings),
    ));
    col = col.push(
        column![
            text(tr(app.lang, "footer.license")).size(12).style(muted()),
            text(format!("v{}", env!("CARGO_PKG_VERSION")))
                .size(12)
                .style(muted()),
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
        .style(move |_| sidebar_style())
        .into()
}

/// The i18n key of a file tool, via the sidebar's own table.
fn file_tool_key(tool: FileTool) -> &'static str {
    FILE_TABS
        .iter()
        .find(|(t, _)| *t == tool)
        .map_or("", |(_, k)| k)
}

/// The i18n key of a developer tool, via the sidebar's own table.
fn dev_tool_key(tool: DevTool) -> &'static str {
    DEV_TABS
        .iter()
        .find(|(t, _)| *t == tool)
        .map_or("", |(_, k)| k)
}

/// Section → tool breadcrumb for the pages hosting several tools:
/// the section stays muted, the active tool is in the normal text
/// color and reads as "where exactly you are".
fn breadcrumb<'a>(
    app: &'a App,
    section_key: &'static str,
    tool_key: &'static str,
) -> Element<'a, Msg> {
    row![
        text(tr(app.lang, section_key)).size(20).style(muted()),
        text("→").size(16).style(muted()),
        text(tr(app.lang, tool_key)).size(20),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

fn topbar(app: &App) -> Element<'_, Msg> {
    let title: Element<'_, Msg> = match app.page {
        Page::Files => breadcrumb(app, "nav.files", file_tool_key(app.file_tool)),
        Page::Developer => breadcrumb(app, "nav.developer", dev_tool_key(app.dev_tool)),
        Page::Home => text(tr(app.lang, "nav.home")).size(20).into(),
        Page::History => text(tr(app.lang, "nav.history")).size(20).into(),
        Page::System => text(tr(app.lang, "nav.system")).size(20).into(),
        Page::Support => text(tr(app.lang, "nav.support")).size(20).into(),
        Page::Settings => text(tr(app.lang, "nav.settings")).size(20).into(),
    };
    row![
        title,
        iced::widget::horizontal_space(),
        iced::widget::pick_list(Lang::ALL, Some(app.lang), Msg::SetLang)
            .text_size(12)
            .width(Length::Shrink)
            .style(pick_field_style)
            .menu_style(pick_menu_style),
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
        Page::Settings => settings::view(app),
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
        centered.into()
    } else {
        column![scrollable(
            container(content)
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

    // Thin progress strip under the top bar while a long operation
    // runs (checksum hashing, duplicate scanning).
    let head: Element<Msg> = match app.progress {
        Some(frac) => column![
            topbar(app),
            iced::widget::progress_bar(0.0..=1.0, frac.clamp(0.0, 1.0))
                .height(3.0)
                .width(Length::Fill),
        ]
        .spacing(0)
        .into(),
        None => topbar(app),
    };

    row![
        sidebar(app),
        column![head, body].width(Length::Fill).height(Length::Fill)
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

#[cfg(test)]
mod nav_section_tests {
    use crate::app::{DevTool, FileTool, NavReveal, Page};

    use super::{dev_tool_key, file_tool_key, section_stack_height};

    #[test]
    fn breadcrumb_keys_resolve_through_the_tables() {
        assert_eq!(file_tool_key(FileTool::Checksum), "tool.checksum");
        assert_eq!(file_tool_key(FileTool::Renamer), "tool.renamer");
        assert_eq!(dev_tool_key(DevTool::Diff), "dev.diff");
    }

    const FULL: f32 = 136.0;

    fn reveal(page: Page, opening: bool, t: f32) -> Option<NavReveal> {
        Some(NavReveal { page, opening, t })
    }

    #[test]
    fn a_collapsed_section_hides_the_stack_but_keeps_the_header() {
        // The header is rendered separately — this height of 0 must
        // only collapse the sub-item stack (the regression that once
        // swallowed the whole section, header included).
        assert_eq!(section_stack_height(false, None, Page::Files, FULL), 0.0);
    }

    #[test]
    fn an_open_section_shows_the_full_stack() {
        assert_eq!(section_stack_height(true, None, Page::Files, FULL), FULL);
    }

    #[test]
    fn the_reveal_animates_the_height_in_both_directions() {
        let opening = reveal(Page::Files, true, 0.0);
        assert_eq!(
            section_stack_height(true, opening.as_ref(), Page::Files, FULL),
            0.0
        );
        let half = reveal(Page::Files, true, 0.5);
        assert_eq!(
            section_stack_height(true, half.as_ref(), Page::Files, FULL),
            FULL / 2.0
        );
        let closing = reveal(Page::Files, false, 0.5);
        assert_eq!(
            section_stack_height(false, closing.as_ref(), Page::Files, FULL),
            FULL / 2.0
        );
        let almost = reveal(Page::Files, false, 0.99);
        let h = section_stack_height(false, almost.as_ref(), Page::Files, FULL);
        assert!(h > 0.0 && h < FULL / 2.0);
    }

    #[test]
    fn another_sections_reveal_does_not_touch_this_stack() {
        let other = reveal(Page::Developer, true, 0.5);
        assert_eq!(
            section_stack_height(true, other.as_ref(), Page::Files, FULL),
            FULL
        );
        assert_eq!(
            section_stack_height(false, other.as_ref(), Page::Files, FULL),
            0.0
        );
    }
}

//! Brand palettes — calm, precise, dashboard-like.
//!
//! The light theme follows the Lithic-style reference: white surfaces on
//! a very light gray background, dark ink text, and a single indigo
//! accent. The dark theme keeps the same structure with darker greys.

use iced::Color;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

pub const INDIGO: Color = rgb(0x4f, 0x46, 0xe5);
pub const INDIGO_LIGHT: Color = rgb(0x63, 0x66, 0xf1);
#[allow(dead_code)]
pub const INDIGO_DARK: Color = rgb(0x37, 0x33, 0xa3);
pub const INDIGO_SURFACE: Color = rgb(0xee, 0xf2, 0xff);

pub const SLATE_50: Color = rgb(0xf8, 0xfa, 0xfc);
pub const SLATE_100: Color = rgb(0xf1, 0xf5, 0xf9);
pub const SLATE_200: Color = rgb(0xe2, 0xe8, 0xf0);
#[allow(dead_code)]
pub const SLATE_300: Color = rgb(0xcb, 0xd5, 0xe1);
pub const SLATE_400: Color = rgb(0x94, 0xa3, 0xb8);
pub const SLATE_500: Color = rgb(0x64, 0x74, 0x8b);
#[allow(dead_code)]
pub const SLATE_600: Color = rgb(0x47, 0x55, 0x69);
#[allow(dead_code)]
pub const SLATE_700: Color = rgb(0x33, 0x41, 0x55);
pub const SLATE_800: Color = rgb(0x1e, 0x29, 0x3b);
pub const SLATE_900: Color = rgb(0x0f, 0x17, 0x2a);

pub const OK: Color = rgb(0x10, 0xb9, 0x81);
pub const OK_DARK: Color = rgb(0x06, 0x95, 0x69);
pub const DANGER: Color = rgb(0xef, 0x44, 0x44);
pub const DANGER_DARK: Color = rgb(0xb9, 0x1c, 0x1c);
pub const WARN: Color = rgb(0xf5, 0x90, 0x18);
pub const WARN_DARK: Color = rgb(0xb4, 0x53, 0x09);

pub struct Scheme {
    pub background: Color,
    pub surface: Color,
    pub surface_hover: Color,
    pub surface_active: Color,
    pub border: Color,
    pub divider: Color,
    pub text: Color,
    pub text_muted: Color,
    pub text_placeholder: Color,
    pub accent: Color,
    pub accent_text: Color,
    pub ok: Color,
    pub danger: Color,
    pub warn: Color,
}

pub const DARK: Scheme = Scheme {
    background: SLATE_900,
    surface: SLATE_800,
    surface_hover: rgb(0x27, 0x35, 0x48),
    surface_active: rgb(0x31, 0x41, 0x55),
    border: rgb(0x3d, 0x4e, 0x64),
    divider: rgb(0x33, 0x45, 0x58),
    text: SLATE_100,
    text_muted: SLATE_400,
    text_placeholder: SLATE_500,
    accent: INDIGO_LIGHT,
    accent_text: rgb(0xff, 0xff, 0xff),
    ok: OK,
    danger: DANGER,
    warn: WARN,
};

pub const LIGHT: Scheme = Scheme {
    background: SLATE_50,
    surface: rgb(0xff, 0xff, 0xff),
    surface_hover: SLATE_100,
    surface_active: INDIGO_SURFACE,
    border: SLATE_200,
    divider: SLATE_200,
    text: SLATE_800,
    text_muted: SLATE_500,
    text_placeholder: SLATE_400,
    accent: INDIGO,
    accent_text: rgb(0xff, 0xff, 0xff),
    ok: OK_DARK,
    danger: DANGER_DARK,
    warn: WARN_DARK,
};

pub fn scheme(dark: bool) -> &'static Scheme {
    if dark {
        &DARK
    } else {
        &LIGHT
    }
}

/// Palette for the built-in iced theme engine.
pub fn palette(dark: bool) -> iced::theme::Palette {
    let scheme = scheme(dark);
    iced::theme::Palette {
        background: scheme.background,
        text: scheme.text,
        primary: scheme.accent,
        success: scheme.ok,
        danger: scheme.danger,
    }
}

//! Brand palettes — calm, precise, dashboard-like.
//!
//! The light theme follows the Lithic-style reference: white surfaces on
//! a very light gray background, dark ink text, and a single accent the
//! user can override (Settings → Appearance, a `#RRGGBB` hex value).
//! The dark theme keeps the same structure with darker greys and a
//! slightly lightened accent for contrast on dark surfaces.

use std::sync::RwLock;

use iced::Color;

pub const fn rgb8(r: u8, g: u8, b: u8) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

pub const INDIGO: Color = rgb8(0x4f, 0x46, 0xe5);

pub const SLATE_50: Color = rgb8(0xf8, 0xfa, 0xfc);
pub const SLATE_100: Color = rgb8(0xf1, 0xf5, 0xf9);
pub const SLATE_200: Color = rgb8(0xe2, 0xe8, 0xf0);
pub const SLATE_400: Color = rgb8(0x94, 0xa3, 0xb8);
pub const SLATE_500: Color = rgb8(0x64, 0x74, 0x8b);
pub const SLATE_800: Color = rgb8(0x1e, 0x29, 0x3b);
pub const SLATE_900: Color = rgb8(0x0f, 0x17, 0x2a);

pub const OK: Color = rgb8(0x10, 0xb9, 0x81);
pub const OK_DARK: Color = rgb8(0x06, 0x95, 0x69);
pub const DANGER: Color = rgb8(0xef, 0x44, 0x44);
pub const DANGER_DARK: Color = rgb8(0xb9, 0x1c, 0x1c);
pub const WARN: Color = rgb8(0xf5, 0x90, 0x18);
pub const WARN_DARK: Color = rgb8(0xb4, 0x53, 0x09);

/// Accent presets offered in Settings → Appearance.
pub const ACCENT_PRESETS: [(&str, Color); 4] = [
    ("#4F46E5", rgb8(0x4f, 0x46, 0xe5)),
    ("#0D9488", rgb8(0x0d, 0x94, 0x88)),
    ("#D97706", rgb8(0xd9, 0x77, 0x06)),
    ("#E11D48", rgb8(0xe1, 0x1d, 0x48)),
];

/// The user accent, global so every view helper reads it without
/// threading the value through a dozen closures. Replaced on startup
/// from the saved setting and live while the user types a new hex.
static ACCENT: RwLock<Color> = RwLock::new(INDIGO);

pub fn set_accent(color: Color) {
    if let Ok(mut slot) = ACCENT.write() {
        *slot = color;
    }
}

pub fn accent() -> Color {
    ACCENT.read().map(|c| *c).unwrap_or(INDIGO)
}

/// Parse `#RRGGBB` (the `#` is optional, case-insensitive).
pub fn parse_hex(s: &str) -> Option<Color> {
    let hex = s.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(hex, 16).ok()?;
    Some(Color {
        r: ((v >> 16) & 0xFF) as f32 / 255.0,
        g: ((v >> 8) & 0xFF) as f32 / 255.0,
        b: (v & 0xFF) as f32 / 255.0,
        a: 1.0,
    })
}

/// Format back to `#RRGGBB` (uppercase, for the settings input).
pub fn to_hex(c: Color) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        (c.r * 255.0).round() as u8,
        (c.g * 255.0).round() as u8,
        (c.b * 255.0).round() as u8
    )
}

/// Mix `c` into `base` at ratio `t` (0 → base, 1 → c).
fn mix(base: Color, c: Color, t: f32) -> Color {
    Color {
        r: base.r + (c.r - base.r) * t,
        g: base.g + (c.g - base.g) * t,
        b: base.b + (c.b - base.b) * t,
        a: 1.0,
    }
}

/// Lighten toward white by `t`.
fn lighten(c: Color, t: f32) -> Color {
    mix(c, Color::WHITE, t)
}

/// Readable text color on top of `c` (relative luminance check).
fn on_color(c: Color) -> Color {
    let lum = 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
    if lum > 0.55 {
        SLATE_900
    } else {
        rgb8(0xff, 0xff, 0xff)
    }
}

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

pub fn scheme(dark: bool) -> Scheme {
    let accent = accent();
    if dark {
        Scheme {
            background: SLATE_900,
            surface: SLATE_800,
            surface_hover: rgb8(0x27, 0x35, 0x48),
            surface_active: rgb8(0x31, 0x41, 0x55),
            border: rgb8(0x3d, 0x4e, 0x64),
            divider: rgb8(0x33, 0x45, 0x58),
            text: SLATE_100,
            text_muted: SLATE_400,
            text_placeholder: SLATE_500,
            accent: lighten(accent, 0.12),
            accent_text: on_color(accent),
            ok: OK,
            danger: DANGER,
            warn: WARN,
        }
    } else {
        Scheme {
            background: SLATE_50,
            surface: rgb8(0xff, 0xff, 0xff),
            surface_hover: SLATE_100,
            surface_active: mix(rgb8(0xff, 0xff, 0xff), accent, 0.10),
            border: SLATE_200,
            divider: SLATE_200,
            text: SLATE_800,
            text_muted: SLATE_500,
            text_placeholder: SLATE_400,
            accent,
            accent_text: on_color(accent),
            ok: OK_DARK,
            danger: DANGER_DARK,
            warn: WARN_DARK,
        }
    }
}

impl Scheme {
    /// A translucent danger overlay for hover/press feedback.
    pub fn danger_tint(&self, alpha: f32) -> Color {
        Color {
            r: self.danger.r,
            g: self.danger.g,
            b: self.danger.b,
            a: alpha,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        let c = parse_hex("#11AA11").expect("valid hex");
        assert!((c.g - 170.0 / 255.0).abs() < 0.001, "AA = 170/255");
        assert_eq!(to_hex(c), "#11AA11");
        assert!(parse_hex("11AA11").is_some(), "# is optional");
        assert!(parse_hex("#11A").is_none(), "3 digits rejected");
        assert!(parse_hex("#11AA1G").is_none(), "non-hex rejected");
        assert!(parse_hex("").is_none());
    }

    #[test]
    fn scheme_uses_the_global_accent() {
        // Amber: luminance below the 0.55 threshold — white text.
        set_accent(rgb8(0xd9, 0x77, 0x06));
        let light = scheme(false);
        assert_eq!(light.accent, rgb8(0xd9, 0x77, 0x06));
        assert_eq!(light.accent_text, rgb8(0xff, 0xff, 0xff));
        // A genuinely light accent flips the text to dark ink.
        set_accent(rgb8(0xfa, 0xcc, 0x15));
        assert_eq!(scheme(false).accent_text, SLATE_900);
        // Dark surfaces lighten the accent for contrast.
        set_accent(INDIGO);
        let dark = scheme(true);
        assert_eq!(dark.accent, lighten(INDIGO, 0.12));
        assert_eq!(dark.accent_text, rgb8(0xff, 0xff, 0xff));
    }
}

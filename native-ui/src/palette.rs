//! The user-owned palette — calm, precise, dashboard-like.
//!
//! There are no themes: five user colors (accent, background, sidebar
//! panel, cards, text) define the whole look, and everything else is
//! derived — hover/active surfaces, borders, dividers, muted text — so
//! any recolor stays coherent. Each color is editable in Settings →
//! Appearance as a `#RRGGBB` hex and persisted across restarts.

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

/// The fixed brand color of the logo mark — independent of the user
/// accent, so the logo never changes.
pub const BRAND_INDIGO: Color = rgb8(0x4f, 0x46, 0xe5);

pub const OK: Color = rgb8(0x10, 0xb9, 0x81);
pub const DANGER: Color = rgb8(0xef, 0x44, 0x44);
pub const WARN: Color = rgb8(0xf5, 0x90, 0x18);

/// The five colors the user can set individually.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UserColors {
    /// Buttons, links, progress, selection highlights.
    pub accent: Color,
    /// App background.
    pub background: Color,
    /// Sidebar panel.
    pub panel: Color,
    /// Cards and content surfaces.
    pub surface: Color,
    /// Primary text.
    pub text: Color,
}

/// Factory default — the classic dark look.
impl Default for UserColors {
    fn default() -> Self {
        Self {
            accent: rgb8(0x4f, 0x46, 0xe5),
            background: rgb8(0x0f, 0x17, 0x2a),
            panel: rgb8(0x0f, 0x17, 0x2a),
            surface: rgb8(0x1e, 0x29, 0x3b),
            text: rgb8(0xf1, 0xf5, 0xf9),
        }
    }
}

impl UserColors {
    /// The classic light look, as a one-click preset.
    pub fn light() -> Self {
        Self {
            accent: rgb8(0x4f, 0x46, 0xe5),
            background: rgb8(0xf8, 0xfa, 0xfc),
            panel: rgb8(0xf8, 0xfa, 0xfc),
            surface: rgb8(0xff, 0xff, 0xff),
            text: rgb8(0x1e, 0x29, 0x3b),
        }
    }
}

/// Which of the five user colors a settings row edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorSlot {
    Accent,
    Background,
    Panel,
    Surface,
    Text,
}

impl ColorSlot {
    pub const ALL: [ColorSlot; 5] = [
        ColorSlot::Accent,
        ColorSlot::Background,
        ColorSlot::Panel,
        ColorSlot::Surface,
        ColorSlot::Text,
    ];

    pub fn key(self) -> &'static str {
        match self {
            ColorSlot::Accent => "color_accent",
            ColorSlot::Background => "color_background",
            ColorSlot::Panel => "color_panel",
            ColorSlot::Surface => "color_surface",
            ColorSlot::Text => "color_text",
        }
    }

    pub fn get(self, colors: &UserColors) -> Color {
        match self {
            ColorSlot::Accent => colors.accent,
            ColorSlot::Background => colors.background,
            ColorSlot::Panel => colors.panel,
            ColorSlot::Surface => colors.surface,
            ColorSlot::Text => colors.text,
        }
    }

    /// The settings-page label key for this slot.
    pub fn i18n_key(self) -> &'static str {
        match self {
            ColorSlot::Accent => "settings.color.accent",
            ColorSlot::Background => "settings.color.background",
            ColorSlot::Panel => "settings.color.sidebar",
            ColorSlot::Surface => "settings.color.cards",
            ColorSlot::Text => "settings.color.text",
        }
    }

    pub fn set(self, colors: &mut UserColors, c: Color) {
        match self {
            ColorSlot::Accent => colors.accent = c,
            ColorSlot::Background => colors.background = c,
            ColorSlot::Panel => colors.panel = c,
            ColorSlot::Surface => colors.surface = c,
            ColorSlot::Text => colors.text = c,
        }
    }
}

/// The user palette, global so every view helper reads it without
/// threading values through a dozen closures.
static USER_COLORS: RwLock<UserColors> = RwLock::new(UserColors {
    accent: rgb8(0x4f, 0x46, 0xe5),
    background: rgb8(0x0f, 0x17, 0x2a),
    panel: rgb8(0x0f, 0x17, 0x2a),
    surface: rgb8(0x1e, 0x29, 0x3b),
    text: rgb8(0xf1, 0xf5, 0xf9),
});

pub fn set_user_colors(colors: UserColors) {
    if let Ok(mut slot) = USER_COLORS.write() {
        *slot = colors;
    }
}

pub fn set_color(slot: ColorSlot, c: Color) {
    if let Ok(mut colors) = USER_COLORS.write() {
        slot.set(&mut colors, c);
    }
}

/// Hex input texts for all slots, from the current palette.
pub fn color_inputs() -> std::collections::HashMap<ColorSlot, String> {
    ColorSlot::ALL
        .iter()
        .map(|slot| (*slot, to_hex(slot.get(&user_colors()))))
        .collect()
}

pub fn user_colors() -> UserColors {
    USER_COLORS.read().map(|c| *c).unwrap_or_default()
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

/// Format back to `#RRGGBB` (uppercase, for the settings inputs).
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

/// Readable text color on top of `c` (relative luminance check).
pub fn on_color_for(c: Color) -> Color {
    let lum = 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
    if lum > 0.55 {
        rgb8(0x0f, 0x17, 0x2a)
    } else {
        rgb8(0xff, 0xff, 0xff)
    }
}

pub struct Scheme {
    pub background: Color,
    pub panel: Color,
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

/// Derive the full scheme from the five user colors.
pub fn scheme() -> Scheme {
    let u = user_colors();
    Scheme {
        background: u.background,
        panel: u.panel,
        surface: u.surface,
        surface_hover: mix(u.surface, u.text, 0.08),
        surface_active: mix(u.surface, u.accent, 0.14),
        border: mix(u.surface, u.text, 0.22),
        divider: mix(u.surface, u.text, 0.15),
        text: u.text,
        text_muted: mix(u.text, u.background, 0.45),
        text_placeholder: mix(u.text, u.background, 0.60),
        accent: u.accent,
        accent_text: on_color_for(u.accent),
        ok: OK,
        danger: DANGER,
        warn: WARN,
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
pub fn iced_palette() -> iced::theme::Palette {
    let scheme = scheme();
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
    fn slot_set_and_get() {
        let mut colors = UserColors::default();
        ColorSlot::Panel.set(&mut colors, rgb8(0x11, 0x22, 0x33));
        assert_eq!(ColorSlot::Panel.get(&colors), rgb8(0x11, 0x22, 0x33));
        assert_eq!(ColorSlot::Accent.get(&colors), rgb8(0x4f, 0x46, 0xe5));
    }

    #[test]
    fn derived_colors_follow_the_user_palette() {
        set_user_colors(UserColors::default());
        let s = scheme();
        // Derived hover sits between surface and text.
        assert_eq!(
            s.surface_hover,
            mix(rgb8(0x1e, 0x29, 0x3b), rgb8(0xf1, 0xf5, 0xf9), 0.08)
        );
        // Muted text sits between text and background.
        assert_eq!(
            s.text_muted,
            mix(rgb8(0xf1, 0xf5, 0xf9), rgb8(0x0f, 0x17, 0x2a), 0.45)
        );
        // Light preset: hover moves the other way, still coherent.
        set_user_colors(UserColors::light());
        let s = scheme();
        assert_eq!(
            s.surface_hover,
            mix(rgb8(0xff, 0xff, 0xff), rgb8(0x1e, 0x29, 0x3b), 0.08)
        );
        assert_eq!(s.accent_text, rgb8(0xff, 0xff, 0xff));
        // A genuinely light accent flips the button text to dark ink.
        set_color(ColorSlot::Accent, rgb8(0xfa, 0xcc, 0x15));
        assert_eq!(scheme().accent_text, rgb8(0x0f, 0x17, 0x2a));
        set_user_colors(UserColors::default());
    }
}

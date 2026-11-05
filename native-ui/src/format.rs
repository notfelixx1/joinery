//! Formatting helpers — port of the frontend's format.js.

const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];

/// Human-readable byte size, locale-independent.
pub fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    let text = if unit == 0 {
        format!("{}", bytes)
    } else if value >= 100.0 {
        format!("{:.0}", value)
    } else {
        format!("{:.1}", value)
    };
    format!("{} {}", text, UNITS[unit])
}

/// Percentage clamped to [0, 100].
pub fn percent(part: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        ((part as f64 / total as f64) * 100.0).min(100.0) as f32
    }
}

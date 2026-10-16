//! Hand-rolled palette + tiny animation math helpers.
//!
//! Everything is painted with explicit colors (no egui widget theming), so
//! the launcher keeps one consistent look across host and device.

use eframe::egui::Color32;

/// `0xRRGGBB` -> opaque color.
pub fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    )
}

pub fn bg() -> Color32 {
    rgb(0x0B0B0F)
}

pub fn surface() -> Color32 {
    rgb(0x16161D)
}

pub fn surface_hi() -> Color32 {
    rgb(0x22222E)
}

pub fn text() -> Color32 {
    rgb(0xF2F2F7)
}

pub fn subtext() -> Color32 {
    rgb(0x9AA0A6)
}

pub fn accent() -> Color32 {
    rgb(0xE8452C)
}

pub fn ok() -> Color32 {
    rgb(0x4CC38A)
}

/// Multiply the alpha channel of a color.
pub fn with_alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a.clamp(0.0, 1.0) * 255.0) as u8)
}

/// Linear interpolation for `t` in `0..=1`.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// Frame-rate independent exponential approach towards `target`.
pub fn approach(cur: f32, target: f32, dt: f32, speed: f32) -> f32 {
    cur + (target - cur) * (1.0 - (-(speed * dt)).exp())
}

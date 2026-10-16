//! Palette, typography and small animation helpers.
//!
//! Everything is painted with explicit colors (no egui widget theming) so the
//! launcher looks the same on device and in the desktop preview. The three
//! Roboto weights are bundled (Apache-2.0, see `assets/fonts/LICENSE.txt`) —
//! Light for the big clock, Medium for headings/caps text, Regular for labels.

use std::sync::Arc;

use eframe::egui::{Color32, Context, FontData, FontDefinitions, FontFamily, FontId};

/// `0xRRGGBB` -> opaque color.
pub fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    )
}

/// `0xRRGGBB` -> color with the given alpha (0..=1).
pub fn rgba(hex: u32, a: f32) -> Color32 {
    let c = rgb(hex);
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a.clamp(0.0, 1.0) * 255.0) as u8)
}

/// Multiply the alpha channel of a color.
pub fn with_alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a.clamp(0.0, 1.0) * 255.0) as u8)
}

pub fn black(a: f32) -> Color32 {
    Color32::from_black_alpha((a.clamp(0.0, 1.0) * 255.0) as u8)
}

pub fn white(a: f32) -> Color32 {
    Color32::from_white_alpha((a.clamp(0.0, 1.0) * 255.0) as u8)
}

// ------------------------------------------------------------------ fonts

fn family(name: &str) -> FontFamily {
    FontFamily::Name(Arc::from(name))
}

pub fn font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn font_light(size: f32) -> FontId {
    FontId::new(size, family("robo_light"))
}

pub fn font_medium(size: f32) -> FontId {
    FontId::new(size, family("robo_medium"))
}

/// Install the bundled Roboto weights (called once from `LauncherLab::new`).
pub fn install_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "robo".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/Roboto-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "robo_light".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/Roboto-Light.ttf"
        ))),
    );
    fonts.font_data.insert(
        "robo_medium".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/Roboto-Medium.ttf"
        ))),
    );

    fonts.families.insert(
        FontFamily::Proportional,
        vec!["robo".to_owned(), "robo_medium".to_owned()],
    );
    fonts.families.insert(
        FontFamily::Monospace,
        vec!["robo".to_owned(), "robo_medium".to_owned()],
    );
    fonts.families.insert(
        family("robo_light"),
        vec!["robo_light".to_owned(), "robo".to_owned()],
    );
    fonts.families.insert(
        family("robo_medium"),
        vec!["robo_medium".to_owned(), "robo".to_owned()],
    );
    ctx.set_fonts(fonts);
}

// ---------------------------------------------------------------- palette

/// Colors of the chrome that follows the system theme (pill, drawer, shelf).
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// Google search pill on the workspace.
    pub pill: Color32,
    /// The faint Lens mark inside the pill.
    pub lens: Color32,
    /// Drawer / shelf / hidden-space panel background.
    pub panel: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,
    /// Recents card surface + its hairlines.
    pub card: Color32,
    pub card_line: Color32,
    /// "Search apps" field in the drawer.
    pub search_bg: Color32,
    pub divider: Color32,
    pub toast_bg: Color32,
    /// OnePlus launcher accent (`oneplus_accent_color` in the decoded resources).
    pub accent: Color32,
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            pill: rgba(0x0E0E10, 0.97),
            lens: white(0.30),
            panel: rgba(0x121214, 0.99),
            text: Color32::WHITE,
            text_dim: white(0.70),
            text_faint: white(0.40),
            card: rgb(0x1D1D21),
            card_line: white(0.08),
            search_bg: white(0.08),
            divider: white(0.10),
            toast_bg: rgba(0x2A2A2E, 0.92),
            accent: rgb(0x2196F3),
        }
    } else {
        Palette {
            pill: rgba(0xFFFFFF, 0.96),
            lens: black(0.28),
            panel: rgba(0xFAFAFA, 0.99),
            text: rgb(0x1A1A1A),
            text_dim: black(0.60),
            text_faint: black(0.35),
            card: rgb(0xFFFFFF),
            card_line: black(0.08),
            search_bg: black(0.06),
            divider: black(0.08),
            toast_bg: rgba(0x2A2A2E, 0.92),
            accent: rgb(0x1A73E8),
        }
    }
}

/// OnePlus red used for the clock widget's accent digit.
pub fn oneplus_red() -> Color32 {
    rgb(0xEB0029)
}

/// Frame-rate independent approach towards `target`.
pub fn approach(cur: f32, target: f32, dt: f32, speed: f32) -> f32 {
    let k = 1.0 - (-speed * dt).exp();
    cur + (target - cur) * k
}

//! Procedural stand-in for the wallpaper bundled with OnePlus Launcher 4.4.8
//! (`res/drawable-nodpi/wallpaper_0.jpg`): a painterly abstract made of soft
//! blue/red/violet clouds over a deep indigo base.
//!
//! Nothing from the original bitmap is shipped — every pixel is drawn with
//! overlapping translucent circles, using a tiny deterministic PRNG so the
//! result is stable from frame to frame.

use eframe::egui::{pos2, vec2, Color32, CornerRadius, Painter, Rect};

use crate::theme;

/// Deterministic LCG — only used to jitter cloud puffs.
struct Rng(u32);

impl Rng {
    fn new(seed: u32) -> Self {
        Self(seed.wrapping_mul(2654435761).wrapping_add(1))
    }

    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        ((self.0 >> 8) & 0x00FF_FFFF) as f32 / 16_777_216.0
    }
}

struct Cloud {
    /// Center as fractions of the rectangle.
    cx: f32,
    cy: f32,
    /// Radius as a fraction of the rectangle width.
    r: f32,
    rgb: u32,
    /// Core alpha (0..=1).
    a: f32,
    seed: u32,
}

/// The composition, tuned against the bundled wallpaper's vibe:
/// light patch top-left, big red cloud upper-center-right, blue mid-left,
/// everything sinking into deep navy at the bottom.
const CLOUDS: &[Cloud] = &[
    Cloud { cx: 0.22, cy: 0.06, r: 0.42, rgb: 0xE9E3EC, a: 0.16, seed: 1 },
    Cloud { cx: 0.72, cy: 0.03, r: 0.30, rgb: 0xC9BFD6, a: 0.12, seed: 2 },
    Cloud { cx: 0.70, cy: 0.30, r: 0.50, rgb: 0xD2402F, a: 0.20, seed: 3 },
    Cloud { cx: 0.96, cy: 0.13, r: 0.34, rgb: 0xE4593D, a: 0.18, seed: 4 },
    Cloud { cx: 0.05, cy: 0.42, r: 0.40, rgb: 0x2B5CA8, a: 0.15, seed: 5 },
    Cloud { cx: 0.90, cy: 0.42, r: 0.34, rgb: 0xE98973, a: 0.14, seed: 6 },
    Cloud { cx: 0.30, cy: 0.68, r: 0.46, rgb: 0x182752, a: 0.22, seed: 7 },
    Cloud { cx: 0.62, cy: 0.90, r: 0.54, rgb: 0x0E1433, a: 0.24, seed: 8 },
    Cloud { cx: 0.92, cy: 0.72, r: 0.34, rgb: 0x8E5B78, a: 0.12, seed: 9 },
    Cloud { cx: 0.05, cy: 0.94, r: 0.30, rgb: 0x0A0E26, a: 0.22, seed: 10 },
];

/// Paint the wallpaper into `rect`.
pub fn paint(p: &Painter, rect: Rect) {
    // Base: vertical gradient indigo -> near-black navy.
    const STEPS: usize = 48;
    let top = theme::rgb(0x171E3E);
    let bottom = theme::rgb(0x0C1026);
    for i in 0..STEPS {
        let t0 = i as f32 / STEPS as f32;
        let t1 = (i + 1) as f32 / STEPS as f32;
        let c = lerp_color(top, bottom, (t0 + t1) * 0.5);
        let y0 = rect.top() + rect.height() * t0;
        let y1 = rect.top() + rect.height() * t1;
        p.rect_filled(
            Rect::from_min_max(pos2(rect.left(), y0), pos2(rect.right(), y1 + 0.75)),
            CornerRadius::ZERO,
            c,
        );
    }

    let w = rect.width();
    let h = rect.height().max(1.0);

    for cloud in CLOUDS {
        let c = pos2(rect.left() + cloud.cx * w, rect.top() + cloud.cy * h);
        let r = cloud.r * w;
        let mut rng = Rng::new(cloud.seed);
        let base = theme::rgb(cloud.rgb);

        // Solid-ish core first, then jittered puffs fading outwards.
        for _ in 0..6 {
            let a = cloud.a * 0.75;
            p.circle_filled(c, r * 0.34, theme::with_alpha(base, a));
        }
        for _ in 0..26 {
            let ang = rng.next() * std::f32::consts::TAU;
            let dist = rng.next().powf(0.65) * r * 0.66;
            let off = vec2(ang.cos() * dist, ang.sin() * dist * 0.85);
            let pr = r * (0.26 + rng.next() * 0.36);
            let a = cloud.a * (0.20 + rng.next() * 0.30);
            p.circle_filled(c + off, pr, theme::with_alpha(base, a));
        }
    }
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

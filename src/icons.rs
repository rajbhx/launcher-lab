//! Hand-drawn app icons.
//!
//! No bitmap assets ship with the app: every icon is composed from circles,
//! rounded rectangles, arcs and polygons at paint time, scaled from the
//! 56 dp icon size of the original `device_profiles.xml` ("Large Phone").
//!
//! Coordinates below are expressed in units of the icon *radius* (1.0 = the
//! edge of the icon disc), so the same routines render at any size.

use eframe::egui::{pos2, vec2, Align2, Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke};

use crate::theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum App {
    Phone,
    Messages,
    Chrome,
    Camera,
    Weather,
    Gallery,
    PlayStore,
    Google,
    Gmail,
    Maps,
    YouTube,
    Drive,
    PlayMusic,
    PlayMovies,
    GooglePay,
    Duo,
    Photos,
    Netflix,
    Settings,
    Files,
    Music,
    Notes,
    Clock,
    Calculator,
    Recorder,
    Forums,
    Backup,
}

pub const DOCK: [App; 5] = [
    App::Phone,
    App::Messages,
    App::Chrome,
    App::Camera,
    App::Weather,
];

/// Drawer grid, alphabetical like the original all-apps list.
pub const DRAWER: [App; 27] = [
    App::Backup,
    App::Calculator,
    App::Camera,
    App::Chrome,
    App::Clock,
    App::Drive,
    App::Duo,
    App::Files,
    App::Forums,
    App::Gallery,
    App::Gmail,
    App::Google,
    App::GooglePay,
    App::Maps,
    App::Messages,
    App::Music,
    App::Netflix,
    App::Notes,
    App::Phone,
    App::Photos,
    App::PlayMovies,
    App::PlayMusic,
    App::PlayStore,
    App::Recorder,
    App::Settings,
    App::Weather,
    App::YouTube,
];

/// "Google" folder contents (`default_workspace_5x5_7.xml`).
pub const GOOGLE_FOLDER: [App; 11] = [
    App::Google,
    App::Chrome,
    App::Gmail,
    App::Maps,
    App::YouTube,
    App::Drive,
    App::PlayMusic,
    App::PlayMovies,
    App::Duo,
    App::Photos,
    App::GooglePay,
];

/// "OnePlus" folder contents.
pub const ONEPLUS_FOLDER: [App; 5] = [
    App::Forums,
    App::Backup,
    App::Files,
    App::Weather,
    App::Settings,
];

/// Shelf "Recent apps" card contents.
pub const SHELF_RECENTS: [App; 10] = [
    App::Chrome,
    App::Gallery,
    App::Netflix,
    App::Camera,
    App::Phone,
    App::Gmail,
    App::Maps,
    App::PlayStore,
    App::Google,
    App::Settings,
];

impl App {
    pub fn name(self) -> &'static str {
        match self {
            App::Phone => "Phone",
            App::Messages => "Messages",
            App::Chrome => "Chrome",
            App::Camera => "Camera",
            App::Weather => "Weather",
            App::Gallery => "Gallery",
            App::PlayStore => "Play Store",
            App::Google => "Google",
            App::Gmail => "Gmail",
            App::Maps => "Maps",
            App::YouTube => "YouTube",
            App::Drive => "Drive",
            App::PlayMusic => "Play Music",
            App::PlayMovies => "Play Movies",
            App::GooglePay => "Google Pay",
            App::Duo => "Duo",
            App::Photos => "Photos",
            App::Netflix => "Netflix",
            App::Settings => "Settings",
            App::Files => "Files",
            App::Music => "Music",
            App::Notes => "Notes",
            App::Clock => "Clock",
            App::Calculator => "Calculator",
            App::Recorder => "Recorder",
            App::Forums => "Forums",
            App::Backup => "Backup",
        }
    }

    /// Tint used by recents cards.
    pub fn accent(self) -> u32 {
        match self {
            App::Phone => 0x2A82F0,
            App::Messages => 0x16B879,
            App::Chrome => 0x4285F4,
            App::Camera => 0x5C6672,
            App::Weather => 0x17B9E8,
            App::Gallery => 0x2BB6A3,
            App::PlayStore => 0x00A0FF,
            App::Google => 0x4285F4,
            App::Gmail => 0xEA4335,
            App::Maps => 0x34A853,
            App::YouTube => 0xE62117,
            App::Drive => 0xFBBC04,
            App::PlayMusic | App::Music => 0xE8452C,
            App::PlayMovies => 0x4285F4,
            App::GooglePay => 0x1A73E8,
            App::Duo => 0x4285F4,
            App::Photos => 0xEA4335,
            App::Netflix => 0xE50914,
            App::Settings => 0x60707C,
            App::Files => 0xF9A825,
            App::Notes => 0xFFC107,
            App::Clock => 0x607D8B,
            App::Calculator => 0xFF7A1A,
            App::Recorder => 0xEB0029,
            App::Forums => 0xE8452C,
            App::Backup => 0x3B78E7,
        }
    }

    /// Draw into a square slot (icon occupies ~87.5 % of the slot, i.e. the
    /// 56 dp default icon inside a 64 dp cell).
    pub fn draw(self, p: &Painter, slot: Rect) {
        let d = slot.width().min(slot.height()) * 0.875;
        self.draw_at(p, slot.center(), d * 0.5);
    }

    pub(crate) fn draw_at(self, p: &Painter, c: Pos2, r: f32) {
        match self {
            App::Phone => phone(p, c, r),
            App::Messages => messages(p, c, r),
            App::Chrome => chrome(p, c, r),
            App::Camera => camera(p, c, r),
            App::Weather => weather(p, c, r),
            App::Gallery => gallery(p, c, r),
            App::PlayStore => play_store(p, c, r),
            App::Google => google(p, c, r),
            App::Gmail => gmail(p, c, r),
            App::Maps => maps(p, c, r),
            App::YouTube => youtube(p, c, r),
            App::Drive => drive(p, c, r),
            App::PlayMusic | App::Music => music(p, c, r),
            App::PlayMovies => play_movies(p, c, r),
            App::GooglePay => google_pay(p, c, r),
            App::Duo => duo(p, c, r),
            App::Photos => photos(p, c, r),
            App::Netflix => netflix(p, c, r),
            App::Settings => settings(p, c, r),
            App::Files => files(p, c, r),
            App::Notes => notes(p, c, r),
            App::Clock => clock(p, c, r),
            App::Calculator => calculator(p, c, r),
            App::Recorder => recorder(p, c, r),
            App::Forums => forums(p, c, r),
            App::Backup => backup(p, c, r),
        }
    }
}

/// Folder icon: white disc with a 2x2 preview of the first four apps.
pub fn draw_folder(p: &Painter, slot: Rect, apps: &[App]) {
    let d = slot.width().min(slot.height()) * 0.875;
    let c = slot.center();
    let r = d * 0.5;
    disc(p, c, r, theme::rgb(0xF5F5F7));
    let mini = d * 0.44;
    let off = d * 0.235;
    for (i, app) in apps.iter().take(4).enumerate() {
        let dx = if i % 2 == 0 { -off } else { off };
        let dy = if i < 2 { -off } else { off };
        let ms = Rect::from_center_size(c + vec2(dx, dy), vec2(mini, mini));
        app.draw(p, ms);
    }
}

/// Generic "letter tile" icon (Hidden Space items).
pub fn draw_letter(p: &Painter, slot: Rect, hex: u32, letter: &str) {
    let d = slot.width().min(slot.height()) * 0.875;
    let c = slot.center();
    let r = d * 0.5;
    disc(p, c, r, theme::rgb(hex));
    p.text(
        c,
        Align2::CENTER_CENTER,
        letter,
        theme::font_medium(d * 0.42),
        Color32::WHITE,
    );
}

// ------------------------------------------------------------------ drawing

pub(crate) fn disc(p: &Painter, c: Pos2, r: f32, col: Color32) {
    p.circle_filled(c, r, col);
}

pub(crate) fn rr(p: &Painter, c: Pos2, w: f32, h: f32, rad: f32, col: Color32) {
    rrect(p, Rect::from_center_size(c, vec2(w, h)), rad, col);
}

pub(crate) fn rrect(p: &Painter, rect: Rect, rad: f32, col: Color32) {
    p.rect_filled(rect, CornerRadius::same(rad.clamp(0.0, 255.0) as u8), col);
}

pub(crate) fn poly(p: &Painter, pts: &[Pos2], col: Color32) {
    p.add(Shape::convex_polygon(pts.to_vec(), col, Stroke::NONE));
}

pub(crate) fn line(p: &Painter, a: Pos2, b: Pos2, w: f32, col: Color32) {
    p.add(Shape::line(vec![a, b], Stroke::new(w, col)));
    let h = w * 0.5;
    p.circle_filled(a, h, col);
    p.circle_filled(b, h, col);
}

pub(crate) fn pline(p: &Painter, pts: &[Pos2], w: f32, col: Color32) {
    p.add(Shape::line(pts.to_vec(), Stroke::new(w, col)));
    let h = w * 0.5;
    for pt in pts {
        p.circle_filled(*pt, h, col);
    }
}

pub(crate) fn arc(p: &Painter, c: Pos2, r: f32, w: f32, a0: f32, a1: f32, col: Color32) {
    let steps = (((a1 - a0).abs() / 7.0).ceil() as usize).max(2);
    let pts: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let a = (a0 + (a1 - a0) * i as f32 / steps as f32).to_radians();
            pos2(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect();
    pline(p, &pts, w, col);
}

pub(crate) fn pie(p: &Painter, c: Pos2, r: f32, a0: f32, a1: f32, col: Color32) {
    let steps = (((a1 - a0).abs() / 7.0).ceil() as usize).max(2);
    let mut pts = vec![c];
    for i in 0..=steps {
        let a = (a0 + (a1 - a0) * i as f32 / steps as f32).to_radians();
        pts.push(pos2(c.x + r * a.cos(), c.y + r * a.sin()));
    }
    poly(p, &pts, col);
}

pub(crate) fn plus(p: &Painter, c: Pos2, arm: f32, w: f32, col: Color32) {
    line(p, c - vec2(arm, 0.0), c + vec2(arm, 0.0), w, col);
    line(p, c - vec2(0.0, arm), c + vec2(0.0, arm), w, col);
}

pub(crate) fn dots3h(p: &Painter, c: Pos2, spacing: f32, rad: f32, col: Color32) {
    for i in -1..=1 {
        p.circle_filled(c + vec2(spacing * i as f32, 0.0), rad, col);
    }
}

pub(crate) fn dots3v(p: &Painter, c: Pos2, spacing: f32, rad: f32, col: Color32) {
    for i in -1..=1 {
        p.circle_filled(c + vec2(0.0, spacing * i as f32), rad, col);
    }
}

/// Magnifier glyph (drawer search field).
pub fn glyph_magnifier(p: &Painter, c: Pos2, s: f32, col: Color32) {
    let r = s * 0.30;
    p.circle_stroke(c + vec2(-s * 0.08, -s * 0.08), r, Stroke::new(s * 0.13, col));
    let a = c + vec2(-s * 0.08, -s * 0.08) + vec2(r * 0.70, r * 0.70);
    let b = a + vec2(s * 0.30, s * 0.30);
    line(p, a, b, s * 0.13, col);
}

// ------------------------------------------------------------------- icons

fn phone(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, theme::rgb(0x2A82F0));
    let u = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    pline(
        p,
        &[
            u(-0.40, -0.30),
            u(-0.50, -0.10),
            u(-0.42, 0.08),
            u(-0.24, 0.20),
            u(-0.04, 0.28),
            u(0.16, 0.37),
            u(0.30, 0.40),
        ],
        r * 0.30,
        Color32::WHITE,
    );
}

fn messages(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, theme::rgb(0x16B879));
    dots3h(p, c, r * 0.34, r * 0.12, Color32::WHITE);
}

fn chrome(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r * 0.97, Color32::WHITE);
    pie(p, c, r * 0.96, -150.0, -30.0, theme::rgb(0xEA4335));
    pie(p, c, r * 0.96, -30.0, 90.0, theme::rgb(0xFBBC05));
    pie(p, c, r * 0.96, 90.0, 210.0, theme::rgb(0x34A853));
    disc(p, c, r * 0.50, Color32::WHITE);
    disc(p, c, r * 0.40, theme::rgb(0x4285F4));
}

fn camera(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, theme::rgb(0xF7F7F8));
    let u = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    rr(p, u(0.0, 0.10), r * 1.10, r * 0.80, r * 0.22, theme::rgb(0x2E3237));
    rr(p, u(0.0, -0.34), r * 0.40, r * 0.18, r * 0.08, theme::rgb(0x2E3237));
    disc(p, u(0.0, 0.10), r * 0.28, Color32::WHITE);
    disc(p, u(0.0, 0.10), r * 0.13, theme::rgb(0x2E3237));
    disc(p, u(-0.40, -0.16), r * 0.07, Color32::WHITE);
}

fn weather(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, theme::rgb(0x17B9E8));
    let u = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    // Rising sun over water.
    line(p, u(-0.24, -0.62), u(0.24, -0.62), r * 0.09, Color32::WHITE);
    line(p, u(-0.16, -0.74), u(0.16, -0.74), r * 0.09, Color32::WHITE);
    disc(p, u(0.0, -0.30), r * 0.22, Color32::WHITE);
    pie(p, u(0.0, 0.10), r * 0.86, 0.0, 180.0, theme::rgb(0x1668C4));
    line(p, u(-0.30, 0.38), u(-0.06, 0.38), r * 0.07, Color32::WHITE);
    line(p, u(0.08, 0.58), u(0.30, 0.58), r * 0.07, Color32::WHITE);
}

fn gallery(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, theme::rgb(0x2BB6A3));
    let u = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    poly(
        p,
        &[u(-0.64, 0.34), u(-0.10, -0.30), u(0.42, 0.34)],
        Color32::WHITE,
    );
    poly(
        p,
        &[u(0.10, 0.34), u(0.42, 0.02), u(0.86, 0.42)],
        Color32::WHITE,
    );
    disc(p, u(0.34, -0.36), r * 0.12, Color32::WHITE);
}

fn play_store(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, Color32::WHITE);
    let u = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    poly(
        p,
        &[u(-0.62, -0.62), u(-0.06, 0.0), u(-0.62, 0.62)],
        theme::rgb(0x00A0FF),
    );
    poly(
        p,
        &[u(-0.62, -0.62), u(0.16, -0.22), u(0.62, 0.0)],
        theme::rgb(0x00E676),
    );
    poly(
        p,
        &[u(-0.62, 0.62), u(0.16, 0.22), u(0.62, 0.0)],
        theme::rgb(0xFFCE00),
    );
    poly(
        p,
        &[u(-0.10, 0.0), u(0.18, -0.16), u(0.62, 0.0), u(0.18, 0.16)],
        theme::rgb(0xFF3A44),
    );
}

fn google(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, Color32::WHITE);
    google_g(p, c, r * 0.55, r * 0.28, r * 0.52);
}

fn google_g(p: &Painter, c: Pos2, rad: f32, w: f32, bar: f32) {
    arc(p, c, rad, w, 205.0, 310.0, theme::rgb(0xEA4335));
    arc(p, c, rad, w, 135.0, 205.0, theme::rgb(0xFBBC05));
    arc(p, c, rad, w, 55.0, 135.0, theme::rgb(0x34A853));
    arc(p, c, rad, w, -35.0, 55.0, theme::rgb(0x4285F4));
    rr(
        p,
        c + vec2(bar * 0.5, 0.16 * rad + w * 0.2),
        bar,
        w * 0.97,
        w * 0.15,
        theme::rgb(0x4285F4),
    );
}

fn gmail(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, Color32::WHITE);
    let u = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    rr(p, u(0.0, 0.0), r * 1.06, r * 0.76, r * 0.12, theme::rgb(0xEA4335));
    rr(p, u(0.0, 0.0), r * 0.90, r * 0.60, r * 0.08, Color32::WHITE);
    pline(
        p,
        &[u(-0.40, -0.22), u(0.0, 0.14), u(0.40, -0.22)],
        r * 0.15,
        theme::rgb(0xEA4335),
    );
}

fn maps(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, Color32::WHITE);
    pie(p, c, r * 0.92, 90.0, 180.0, theme::rgb(0x34A853));
    pie(p, c, r * 0.92, 0.0, 90.0, theme::rgb(0xFBBC04));
    let u = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    poly(
        p,
        &[u(-0.24, -0.10), u(0.24, -0.10), u(0.0, 0.30)],
        theme::rgb(0xEA4335),
    );
    disc(p, u(0.0, -0.24), r * 0.30, theme::rgb(0xEA4335));
    disc(p, u(0.0, -0.24), r * 0.12, Color32::WHITE);
}

fn youtube(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, Color32::WHITE);
    let u = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    rr(p, u(0.0, 0.0), r * 1.28, r * 0.90, r * 0.28, theme::rgb(0xFF0000));
    poly(
        p,
        &[u(-0.16, -0.24), u(-0.16, 0.24), u(0.26, 0.0)],
        Color32::WHITE,
    );
}

fn drive(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, Color32::WHITE);
    let u = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    poly(
        p,
        &[u(-0.62, 0.34), u(0.0, -0.62), u(0.0, 0.34)],
        theme::rgb(0xFBBC04),
    );
    poly(
        p,
        &[u(0.0, -0.62), u(0.62, 0.34), u(0.0, 0.34)],
        theme::rgb(0x34A853),
    );
    poly(
        p,
        &[u(-0.62, 0.34), u(0.62, 0.34), u(0.48, 0.60), u(-0.48, 0.60)],
        theme::rgb(0x4285F4),
    );
}

fn music(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, theme::rgb(0xE8452C));
    let u = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    disc(p, u(-0.26, 0.34), r * 0.24, Color32::WHITE);
    pline(
        p,
        &[u(0.0, 0.34), u(0.0, -0.30), u(0.38, -0.16)],
        r * 0.13,
        Color32::WHITE,
    );
}

fn play_movies(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, Color32::WHITE);
    let cols = [0x4285F4u32, 0xEA4335, 0xFBBC05, 0x34A853];
    for (i, hex) in cols.iter().enumerate() {
        let x = -0.48 + i as f32 * 0.32;
        rr(
            p,
            c + vec2(x * r, 0.0),
            r * 0.22,
            r * 0.96,
            r * 0.08,
            theme::rgb(*hex),
        );
    }
}

fn google_pay(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, Color32::WHITE);
    let u = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    google_g(p, u(0.0, -0.20), r * 0.30, r * 0.16, r * 0.28);
    rr(
        p,
        u(0.0, 0.46),
        r * 0.72,
        r * 0.12,
        r * 0.06,
        theme::rgb(0x1A73E8),
    );
}

fn duo(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, Color32::WHITE);
    let u = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    rr(p, u(0.0, 0.0), r * 1.10, r * 0.84, r * 0.22, theme::rgb(0x4285F4));
    rr(p, u(-0.18, 0.0), r * 0.44, r * 0.32, r * 0.08, Color32::WHITE);
    poly(
        p,
        &[u(0.04, -0.16), u(0.04, 0.16), u(0.36, 0.0)],
        Color32::WHITE,
    );
}

fn photos(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, Color32::WHITE);
    pie(p, c, r * 0.74, 180.0, 270.0, theme::rgb(0xEA4335));
    pie(p, c, r * 0.74, 270.0, 360.0, theme::rgb(0x4285F4));
    pie(p, c, r * 0.74, 90.0, 180.0, theme::rgb(0xFBBC04));
    pie(p, c, r * 0.74, 0.0, 90.0, theme::rgb(0x34A853));
}

fn netflix(p: &Painter, c: Pos2, r: f32) {
    rr(p, c, r * 1.72, r * 1.72, r * 0.34, theme::rgb(0x0B0B0B));
    let u = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    let col = theme::rgb(0xE50914);
    line(p, u(-0.34, -0.52), u(-0.34, 0.52), r * 0.24, col);
    line(p, u(0.34, -0.52), u(0.34, 0.52), r * 0.24, col);
    line(p, u(-0.34, -0.52), u(0.34, 0.52), r * 0.26, col);
}

fn settings(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, theme::rgb(0x4C5560));
    p.circle_stroke(c, r * 0.42, Stroke::new(r * 0.28, Color32::WHITE));
    for k in 0..8 {
        let a = (k as f32 * 45.0 + 22.5).to_radians();
        let a0 = pos2(c.x + a.cos() * r * 0.56, c.y + a.sin() * r * 0.56);
        let a1 = pos2(c.x + a.cos() * r * 0.80, c.y + a.sin() * r * 0.80);
        line(p, a0, a1, r * 0.16, Color32::WHITE);
    }
}

fn files(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, theme::rgb(0xF9A825));
    let u = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    rr(p, u(-0.14, -0.30), r * 0.48, r * 0.28, r * 0.10, Color32::WHITE);
    rr(p, u(0.0, 0.10), r * 0.96, r * 0.66, r * 0.14, Color32::WHITE);
}

fn notes(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, theme::rgb(0xFFD54F));
    let u = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    rr(p, u(0.02, 0.0), r * 0.80, r * 0.96, r * 0.12, Color32::WHITE);
    for i in 0..3 {
        rr(
            p,
            u(-0.20, -0.26 + i as f32 * 0.24),
            r * 0.44,
            r * 0.08,
            r * 0.04,
            theme::rgb(0xF0A500),
        );
    }
}

fn clock(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, Color32::WHITE);
    p.circle_stroke(c, r * 0.62, Stroke::new(r * 0.07, theme::rgb(0xC3C9CE)));
    let u = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    let col = theme::rgb(0x37474F);
    line(p, c, u(0.0, -0.34), r * 0.09, col);
    line(p, c, u(0.26, 0.10), r * 0.09, col);
    disc(p, c, r * 0.07, col);
}

fn calculator(p: &Painter, c: Pos2, r: f32) {
    rr(p, c, r * 1.72, r * 1.72, r * 0.44, theme::rgb(0xFF7A1A));
    let u = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    rr(p, u(0.0, -0.04), r * 0.94, r * 1.16, r * 0.16, Color32::WHITE);
    rr(p, u(0.0, -0.34), r * 0.66, r * 0.22, r * 0.06, theme::rgb(0xFF7A1A));
    for row in 0..3 {
        for col in -1..=1 {
            disc(
                p,
                u(col as f32 * 0.24, 0.04 + row as f32 * 0.22),
                r * 0.07,
                theme::rgb(0xFF7A1A),
            );
        }
    }
}

fn recorder(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, Color32::WHITE);
    let u = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    let col = theme::rgb(0xEB0029);
    rr(p, u(0.0, -0.26), r * 0.30, r * 0.58, r * 0.15, col);
    arc(p, u(0.0, -0.10), r * 0.40, r * 0.09, 20.0, 160.0, col);
    line(p, u(0.0, 0.30), u(0.0, 0.48), r * 0.09, col);
    line(p, u(-0.22, 0.48), u(0.22, 0.48), r * 0.09, col);
}

fn forums(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, theme::rgb(0xE8452C));
    p.text(
        c,
        Align2::CENTER_CENTER,
        "1+",
        theme::font_medium(r * 0.95),
        Color32::WHITE,
    );
}

fn backup(p: &Painter, c: Pos2, r: f32) {
    disc(p, c, r, theme::rgb(0x3B78E7));
    let u = |x: f32, y: f32| pos2(c.x + x * r, c.y + y * r);
    line(p, u(0.0, 0.30), u(0.0, -0.18), r * 0.16, Color32::WHITE);
    poly(
        p,
        &[u(-0.28, -0.10), u(0.28, -0.10), u(0.0, -0.46)],
        Color32::WHITE,
    );
    line(p, u(-0.36, 0.38), u(0.36, 0.38), r * 0.12, Color32::WHITE);
}

// --------------------------------------------------------------- ui glyphs

/// Chevron-left (back).
pub fn glyph_back(p: &Painter, c: Pos2, s: f32, col: Color32) {
    pline(
        p,
        &[
            pos2(c.x + s * 0.30, c.y - s * 0.50),
            pos2(c.x - s * 0.28, c.y),
            pos2(c.x + s * 0.30, c.y + s * 0.50),
        ],
        s * 0.16,
        col,
    );
}

/// Small chevron-right (row affordance).
pub fn glyph_chev(p: &Painter, c: Pos2, s: f32, col: Color32) {
    pline(
        p,
        &[
            pos2(c.x - s * 0.22, c.y - s * 0.36),
            pos2(c.x + s * 0.18, c.y),
            pos2(c.x - s * 0.22, c.y + s * 0.36),
        ],
        s * 0.13,
        col,
    );
}

/// Filled play triangle.
pub fn glyph_play(p: &Painter, c: Pos2, s: f32, col: Color32) {
    poly(
        p,
        &[
            pos2(c.x - s * 0.36, c.y - s * 0.52),
            pos2(c.x + s * 0.54, c.y),
            pos2(c.x - s * 0.36, c.y + s * 0.52),
        ],
        col,
    );
}

/// Pause bars.
pub fn glyph_pause(p: &Painter, c: Pos2, s: f32, col: Color32) {
    rr(p, c + vec2(-s * 0.24, 0.0), s * 0.26, s * 1.0, s * 0.09, col);
    rr(p, c + vec2(s * 0.24, 0.0), s * 0.26, s * 1.0, s * 0.09, col);
}

/// Skip track (triangle + bar), mirrored when `back`.
pub fn glyph_skip(p: &Painter, c: Pos2, s: f32, col: Color32, back: bool) {
    let m = if back { -1.0 } else { 1.0 };
    poly(
        p,
        &[
            pos2(c.x - m * s * 0.44, c.y - s * 0.42),
            pos2(c.x + m * s * 0.16, c.y),
            pos2(c.x - m * s * 0.44, c.y + s * 0.42),
        ],
        col,
    );
    rr(p, c + vec2(m * s * 0.34, 0.0), s * 0.16, s * 0.9, s * 0.06, col);
}

/// Map pin (ring + tail).
pub fn glyph_pin(p: &Painter, c: Pos2, s: f32, col: Color32) {
    p.circle_stroke(
        c + vec2(0.0, -s * 0.22),
        s * 0.34,
        Stroke::new(s * 0.16, col),
    );
    line(p, c + vec2(-s * 0.26, 0.02), c + vec2(0.0, s * 0.62), s * 0.13, col);
    line(p, c + vec2(s * 0.26, 0.02), c + vec2(0.0, s * 0.62), s * 0.13, col);
}

/// Camera shutter (ring + core).
pub fn glyph_shutter(p: &Painter, c: Pos2, r: f32, col: Color32) {
    p.circle_stroke(c, r, Stroke::new(r * 0.22, col));
    disc(p, c, r * 0.55, col);
}

/// Wi-Fi arcs.
pub fn glyph_wifi(p: &Painter, c: Pos2, s: f32, col: Color32) {
    let b = c + vec2(0.0, s * 0.34);
    arc(p, b, s * 0.64, s * 0.16, 205.0, 335.0, col);
    arc(p, b, s * 0.40, s * 0.16, 205.0, 335.0, col);
    disc(p, b, s * 0.10, col);
}

/// Bluetooth rune.
pub fn glyph_bluetooth(p: &Painter, c: Pos2, s: f32, col: Color32) {
    let u = |x: f32, y: f32| c + vec2(x * s, y * s);
    line(p, u(0.0, -0.60), u(0.0, 0.60), s * 0.15, col);
    pline(
        p,
        &[u(0.0, -0.60), u(0.46, -0.22), u(0.0, 0.18)],
        s * 0.15,
        col,
    );
    pline(
        p,
        &[u(0.0, 0.60), u(0.46, 0.22), u(0.0, -0.18)],
        s * 0.15,
        col,
    );
}

/// Flashlight (head + body).
pub fn glyph_flashlight(p: &Painter, c: Pos2, s: f32, col: Color32) {
    let u = |x: f32, y: f32| c + vec2(x * s, y * s);
    poly(
        p,
        &[
            u(-0.30, -0.52),
            u(0.30, -0.52),
            u(0.42, -0.14),
            u(-0.42, -0.14),
        ],
        col,
    );
    rr(p, u(0.0, 0.20), s * 0.44, s * 0.76, s * 0.10, col);
}

/// Paper-plane (airplane mode).
pub fn glyph_airplane(p: &Painter, c: Pos2, s: f32, col: Color32) {
    let u = |x: f32, y: f32| c + vec2(x * s, y * s);
    poly(p, &[u(0.0, -0.55), u(0.50, 0.42), u(0.0, 0.12)], col);
    poly(p, &[u(0.0, -0.55), u(-0.50, 0.42), u(0.0, 0.12)], col);
    poly(p, &[u(0.0, -0.55), u(0.10, 0.02), u(-0.10, 0.02)], col);
}

/// Auto-rotate (circular arrow).
pub fn glyph_rotate(p: &Painter, c: Pos2, s: f32, col: Color32) {
    arc(p, c, s * 0.52, s * 0.15, -60.0, 215.0, col);
    let a = (-60.0f32).to_radians();
    let tip = pos2(c.x + s * 0.52 * a.cos(), c.y + s * 0.52 * a.sin());
    poly(
        p,
        &[
            pos2(tip.x + s * 0.02, tip.y - s * 0.24),
            pos2(tip.x + s * 0.26, tip.y + s * 0.06),
            pos2(tip.x - s * 0.18, tip.y + s * 0.14),
        ],
        col,
    );
}

/// Lightning bolt (battery saver).
pub fn glyph_bolt(p: &Painter, c: Pos2, s: f32, col: Color32) {
    let u = |x: f32, y: f32| c + vec2(x * s, y * s);
    poly(p, &[u(0.14, -0.60), u(-0.34, 0.08), u(-0.02, 0.08)], col);
    poly(p, &[u(-0.14, 0.60), u(0.34, -0.08), u(0.02, -0.08)], col);
}

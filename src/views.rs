//! All rendering. Painter-only: every control is a hand-drawn primitive plus
//! a hit rectangle that input routing in `crate::app` can pick up.
//!
//! The geometry reconstructs the values decoded from the original APK
//! (`res/values/dimens.xml`, `res/xml/device_profiles.xml`,
//! `default_workspace_5x5_7.xml`): 5 columns x 6 rows of 64 dp cells, 15 dp
//! side margins, 18 dp top padding, 66 dp hotseat, status bar drawn over the
//! wallpaper, "^" caret + square dots above the dock.

use eframe::egui::{
    pos2, vec2, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke,
};

use crate::app::LauncherLab;
use crate::clock;
use crate::icons::{self, App};
use crate::model::state::Screen;
use crate::theme::{self, Palette};
use crate::wallpaper;

/// Interactive targets, rebuilt every frame during painting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hit {
    Pill,
    PageDot(usize),
    Caret,
    HomeItem { page: usize, idx: usize },
    Dock(usize),
    FolderClose,
    FolderItem(usize),
    DrawerApp(usize),
    DrawerSearch,
    DrawerMenu,
    HiddenClose,
    HiddenAdd,
    HiddenMenu,
    HiddenItem(usize),
    HiddenHold,
    ShelfTool(usize),
    ShelfApp(usize),
    ShelfAction,
    ShelfNewNote,
    ShelfCard(usize),
    Task(usize),
    TaskIcon(usize),
    TaskClearAll,
    UnlockTap,
}

/// One entry on a home page — mirrors the favorites in
/// `res/xml/default_workspace_5x5_7.xml`.
#[derive(Clone, Copy, Debug)]
pub enum HomeItem {
    App(App),
    Folder {
        title: &'static str,
        apps: &'static [App],
    },
}

/// Items of home page `page` as `(col, row, item)`.
pub fn page_items(page: usize) -> Vec<(usize, usize, HomeItem)> {
    match page {
        0 => vec![
            (
                0,
                4,
                HomeItem::Folder {
                    title: "Google",
                    apps: &icons::GOOGLE_FOLDER,
                },
            ),
            (4, 4, HomeItem::App(App::PlayStore)),
        ],
        1 => vec![
            (1, 0, HomeItem::App(App::Messages)),
            (
                0,
                4,
                HomeItem::Folder {
                    title: "OnePlus",
                    apps: &icons::ONEPLUS_FOLDER,
                },
            ),
            (4, 4, HomeItem::App(App::Netflix)),
        ],
        2 => vec![(2, 0, HomeItem::App(App::Chrome))],
        3 => vec![(3, 0, HomeItem::App(App::Camera))],
        4 => vec![(4, 0, HomeItem::App(App::Gallery))],
        _ => Vec::new(),
    }
}

pub fn item_at(page: usize, idx: usize) -> Option<HomeItem> {
    page_items(page).into_iter().nth(idx).map(|(_, _, it)| it)
}

// ---------------------------------------------------------------- geometry

const STATUS_H: f32 = 30.0;
const TOP_PAD: f32 = 18.0;
const SIDE: f32 = 15.0;
const COLS: usize = 5;
const ROWS: usize = 6;
const ICON: f32 = 64.0;
const NAV_H: f32 = 34.0;
const HOTSEAT_H: f32 = 66.0;

fn cell_w(rect: Rect) -> f32 {
    (rect.width() - 2.0 * SIDE) / COLS as f32
}

fn cell_h(rect: Rect) -> f32 {
    (rect.height() - STATUS_H - TOP_PAD - NAV_H - HOTSEAT_H) / ROWS as f32
}

fn row_cy(rect: Rect, row: usize) -> f32 {
    rect.top() + STATUS_H + TOP_PAD + (row as f32 + 0.5) * cell_h(rect)
}

fn col_cx(rect: Rect, col: usize) -> f32 {
    rect.left() + SIDE + (col as f32 + 0.5) * cell_w(rect)
}

fn slot(rect: Rect, col: usize, row: usize) -> Rect {
    Rect::from_center_size(pos2(col_cx(rect, col), row_cy(rect, row)), vec2(ICON, ICON))
}

fn label(
    p: &Painter,
    pos: Pos2,
    anchor: Align2,
    text: &str,
    font: FontId,
    col: Color32,
    shadow: bool,
) {
    if shadow {
        p.text(pos + vec2(0.0, 1.6), anchor, text, font.clone(), theme::black(0.5));
    }
    p.text(pos, anchor, text, font, col);
}

// ------------------------------------------------------------------- paint

pub fn paint(app: &mut LauncherLab, p: &Painter, rect: Rect) {
    let mut hits = std::mem::take(&mut app.hits);
    let pal = theme::palette(app.theme_dark);

    wallpaper::paint(p, rect);

    draw_home(app, p, rect, &mut hits, &pal);

    if app.screen == Screen::Overview || app.overview_t > 0.001 {
        draw_recents(app, p, rect, &mut hits, &pal);
    }
    let total = if app.shelf_t > 0.001 {
        draw_shelf(app, p, rect, &mut hits, &pal)
    } else {
        app.shelf_content_h
    };
    app.shelf_content_h = total;

    if app.screen == Screen::Hidden || app.hidden_t > 0.001 {
        draw_hidden(app, p, rect, &mut hits, &pal);
    }
    if app.screen == Screen::Drawer || app.drawer_t > 0.001 {
        draw_drawer(app, p, rect, &mut hits, &pal);
    }
    if app.folder_t > 0.001 {
        draw_folder(app, p, rect, &mut hits, &pal);
    }
    if app.lock_t > 0.001 {
        draw_locked(app, p, rect, &mut hits);
    }

    draw_status(app, p, rect);
    if let Some((msg, ttl)) = app.toast.clone() {
        draw_toast(p, rect, &msg, ttl, &pal);
    }

    app.hits = hits;
}

// -------------------------------------------------------------------- home

fn draw_home(app: &LauncherLab, p: &Painter, rect: Rect, hits: &mut Vec<(Rect, Hit)>, pal: &Palette) {
    let w = rect.width();
    let cam = app.pager.current as f32 * w + app.pager.offset;
    for page in 0..app.pager.pages {
        let x = rect.left() - cam + page as f32 * w;
        if x > rect.right() + 1.0 || x + w < rect.left() - 1.0 {
            continue;
        }
        let pr = Rect::from_min_size(pos2(x, rect.top()), vec2(w, rect.height()));
        if page == 0 {
            draw_search_pill(p, pr, pal, hits);
            draw_clock_widget(app, p, pr);
        }
        for (idx, (col, row, item)) in page_items(page).into_iter().enumerate() {
            let s = slot(pr, col, row);
            let hit = Rect::from_min_max(
                pos2(s.center().x - 38.0, s.top() - 4.0),
                pos2(s.center().x + 38.0, s.bottom() + 22.0),
            );
            match item {
                HomeItem::App(a) => {
                    a.draw(p, s);
                    label(
                        p,
                        pos2(s.center().x, s.bottom() + 11.0),
                        Align2::CENTER_CENTER,
                        a.name(),
                        theme::font(13.0),
                        Color32::WHITE,
                        true,
                    );
                }
                HomeItem::Folder { title, apps } => {
                    icons::draw_folder(p, s, apps);
                    label(
                        p,
                        pos2(s.center().x, s.bottom() + 11.0),
                        Align2::CENTER_CENTER,
                        title,
                        theme::font(13.0),
                        Color32::WHITE,
                        true,
                    );
                }
            }
            hits.push((hit, Hit::HomeItem { page, idx }));
        }
    }

    // Dock (no labels) + gesture area.
    let dock_cy = rect.bottom() - NAV_H - HOTSEAT_H * 0.5;
    for (i, app) in icons::DOCK.iter().enumerate() {
        let cx = rect.left() + (i as f32 + 0.5) * w / icons::DOCK.len() as f32;
        let s = Rect::from_center_size(pos2(cx, dock_cy), vec2(ICON, ICON));
        app.draw(p, s);
        hits.push((s.expand(4.0), Hit::Dock(i)));
    }

    // Page indicator: "^" caret + square dots (dots only when > 1 page).
    let cx = rect.center().x;
    let caret_y = rect.bottom() - 126.0;
    p.add(eframe::egui::Shape::line(
        vec![
            pos2(cx - 8.0, caret_y + 4.0),
            pos2(cx, caret_y - 4.0),
            pos2(cx + 8.0, caret_y + 4.0),
        ],
        Stroke::new(2.2, theme::white(0.82)),
    ));
    hits.push((
        Rect::from_center_size(pos2(cx, caret_y), vec2(48.0, 24.0)),
        Hit::Caret,
    ));
    if app.pager.pages > 1 {
        let n = app.pager.pages;
        let dots_y = rect.bottom() - 106.0;
        let total = n as f32 * 4.5 + (n - 1) as f32 * 8.0;
        let x0 = cx - total * 0.5;
        for i in 0..n {
            let a = if i == app.pager.current { 0.92 } else { 0.30 };
            let d = Rect::from_min_size(
                pos2(x0 + i as f32 * 12.5, dots_y - 2.25),
                vec2(4.5, 4.5),
            );
            icons::rrect(p, d, 1.0, theme::white(a));
            hits.push((d.expand2(vec2(6.0, 8.0)), Hit::PageDot(i)));
        }
    }

    // Gesture pill.
    icons::rrect(
        p,
        Rect::from_min_size(
            pos2(rect.center().x - 45.0, rect.bottom() - 16.0),
            vec2(90.0, 4.0),
        ),
        2.0,
        theme::white(0.92),
    );
}

fn draw_search_pill(p: &Painter, pr: Rect, pal: &Palette, hits: &mut Vec<(Rect, Hit)>) {
    let pill = Rect::from_min_size(
        pos2(pr.left() + 28.0, row_cy(pr, 0) - 18.0),
        vec2(pr.width() - 56.0, 48.0),
    );
    icons::rrect(p, pill, 24.0, pal.pill);

    // Google "G".
    let gc = pos2(pill.left() + 21.0, pill.center().y);
    let r = 9.0;
    let w = 3.4;
    icons::arc(p, gc, r, w, 205.0, 310.0, theme::rgb(0xEA4335));
    icons::arc(p, gc, r, w, 135.0, 205.0, theme::rgb(0xFBBC05));
    icons::arc(p, gc, r, w, 55.0, 135.0, theme::rgb(0x34A853));
    icons::arc(p, gc, r, w, -35.0, 55.0, theme::rgb(0x4285F4));
    icons::rrect(
        p,
        Rect::from_min_size(
            pos2(gc.x, gc.y + r * 0.16 - w * 0.5),
            vec2(r * 0.94, w * 0.97),
        ),
        w * 0.2,
        theme::rgb(0x4285F4),
    );

    // Lens mark (very subtle in dark chrome).
    let lc = pill.center();
    p.circle_stroke(lc, 7.5, Stroke::new(1.6, pal.lens));
    p.circle_filled(lc + vec2(2.6, 2.6), 2.2, pal.lens);

    // Multi-color mic.
    let mc = pos2(pill.right() - 21.0, pill.center().y);
    let segs = [0xEA4335u32, 0x4285F4, 0xFBBC05, 0x34A853];
    for (i, hex) in segs.iter().enumerate() {
        icons::rrect(
            p,
            Rect::from_min_size(
                pos2(mc.x - 5.2 + i as f32 * 2.6, mc.y - 9.0),
                vec2(2.6, 11.0),
            ),
            1.2,
            theme::rgb(*hex),
        );
    }
    let white = Color32::WHITE;
    icons::arc(p, pos2(mc.x, mc.y + 1.5), 5.6, 1.7, 0.0, 180.0, white);
    icons::line(p, pos2(mc.x, mc.y + 7.0), pos2(mc.x, mc.y + 9.0), 1.7, white);
    icons::line(
        p,
        pos2(mc.x - 3.0, mc.y + 9.0),
        pos2(mc.x + 3.0, mc.y + 9.0),
        1.7,
        white,
    );

    hits.push((pill, Hit::Pill));
}

fn draw_clock_widget(app: &LauncherLab, p: &Painter, pr: Rect) {
    let x = pr.left() + 28.0;
    let time_bottom = row_cy(pr, 1) + cell_h(pr) * 0.5 - 8.0;
    let t = app.time;
    let time = clock::clock_label(t);

    let font = theme::font_light(46.0);
    let galley = p.layout_no_wrap(time.clone(), font.clone(), Color32::WHITE);
    let tl = pos2(x, time_bottom - galley.size().y);
    let shadow = p.layout_no_wrap(time.clone(), font.clone(), theme::black(0.45));
    p.galley(tl + vec2(0.0, 2.0), shadow, theme::black(0.45));
    p.galley(tl, galley, Color32::WHITE);
    // The leading digit carries the OnePlus accent color, like the stock
    // clock/weather widget on OxygenOS 10.
    let first = time.chars().next().map(|c| c.to_string()).unwrap_or_default();
    let fg = p.layout_no_wrap(first, font, theme::oneplus_red());
    p.galley(tl, fg, theme::oneplus_red());

    let line_font = theme::font_medium(13.0);
    label(
        p,
        pos2(x, time_bottom - 73.0),
        Align2::LEFT_CENTER,
        &clock::date_label(t),
        line_font.clone(),
        theme::white(0.97),
        true,
    );
    label(
        p,
        pos2(x, time_bottom - 52.0),
        Align2::LEFT_CENTER,
        "CLOUDY, 84\u{00B0}",
        line_font,
        theme::white(0.97),
        true,
    );
}

// ------------------------------------------------------------------ folder

fn draw_folder(
    app: &LauncherLab,
    p: &Painter,
    rect: Rect,
    hits: &mut Vec<(Rect, Hit)>,
    pal: &Palette,
) {
    let Some((page, idx)) = app.folder else {
        return;
    };
    let Some(HomeItem::Folder { title, apps }) = item_at(page, idx) else {
        return;
    };
    let t = app.folder_t;
    p.rect_filled(rect, CornerRadius::ZERO, theme::black(0.62 * t));
    // Lowest z first: tapping anywhere else closes the folder.
    hits.push((rect, Hit::FolderClose));

    let rows = apps.len().div_ceil(4);
    let pw = rect.width() - 88.0;
    let ph = 24.0 + rows as f32 * 84.0 + 12.0;
    let panel = Rect::from_min_size(
        pos2(rect.center().x - pw * 0.5, rect.center().y - ph * 0.5 - 26.0),
        vec2(pw, ph),
    );
    icons::rrect(p, panel, 28.0, theme::with_alpha(pal.panel, t));

    let cw = pw / 4.0;
    for (i, a) in apps.iter().enumerate() {
        let cx = panel.left() + (i % 4) as f32 * cw + cw * 0.5;
        let cy = panel.top() + 24.0 + (i / 4) as f32 * 84.0 + 26.0;
        let s = Rect::from_center_size(pos2(cx, cy), vec2(56.0, 56.0));
        a.draw(p, s);
        label(
            p,
            pos2(cx, cy + 42.0),
            Align2::CENTER_CENTER,
            a.name(),
            theme::font(12.0),
            theme::with_alpha(pal.text, 0.85 * t),
            false,
        );
        hits.push((s.expand(10.0), Hit::FolderItem(i)));
    }
    label(
        p,
        pos2(rect.center().x, panel.bottom() + 26.0),
        Align2::CENTER_CENTER,
        title,
        theme::font_medium(16.0),
        theme::white(0.95 * t),
        true,
    );
}

// ------------------------------------------------------------------ drawer

fn draw_drawer(
    app: &LauncherLab,
    p: &Painter,
    rect: Rect,
    hits: &mut Vec<(Rect, Hit)>,
    pal: &Palette,
) {
    let panel = rect.translate(vec2(0.0, (1.0 - app.drawer_t) * rect.height()));
    p.rect_filled(panel, CornerRadius::ZERO, pal.panel);

    // "Search apps" field.
    let field = Rect::from_min_size(
        pos2(panel.left() + 16.0, panel.top() + 44.0),
        vec2(panel.width() - 32.0, 46.0),
    );
    icons::rrect(p, field, 23.0, pal.search_bg);
    icons::glyph_magnifier(
        p,
        pos2(field.left() + 26.0, field.center().y),
        20.0,
        pal.text_dim,
    );
    label(
        p,
        pos2(field.left() + 46.0, field.center().y),
        Align2::LEFT_CENTER,
        "Search apps",
        theme::font(16.0),
        pal.text_dim,
        false,
    );
    icons::dots3v(p, pos2(field.right() - 24.0, field.center().y), 6.5, 2.1, pal.text_dim);
    hits.push((field, Hit::DrawerSearch));
    hits.push((
        Rect::from_min_size(
            pos2(field.right() - 48.0, field.top()),
            vec2(48.0, field.height()),
        ),
        Hit::DrawerMenu,
    ));

    // App grid.
    let grid_top = panel.top() + 118.0 - app.drawer_scroll;
    let slot_w = panel.width() / COLS as f32;
    let row_h = 96.0;
    let rows = icons::DRAWER.len().div_ceil(COLS);
    let content_h = rows as f32 * row_h + 40.0;
    let view_h = (panel.height() - 118.0).max(1.0);
    for (i, a) in icons::DRAWER.iter().enumerate() {
        let cx = panel.left() + (i % COLS) as f32 * slot_w + slot_w * 0.5;
        let cy = grid_top + (i / COLS) as f32 * row_h + 38.0;
        if cy - 38.0 < panel.top() + 112.0 || cy + 44.0 > panel.bottom() - 6.0 {
            continue;
        }
        let s = Rect::from_center_size(pos2(cx, cy), vec2(56.0, 56.0));
        a.draw(p, s);
        label(
            p,
            pos2(cx, cy + 38.0),
            Align2::CENTER_CENTER,
            a.name(),
            theme::font(13.0),
            theme::with_alpha(pal.text, 0.86),
            false,
        );
        hits.push((s.expand(12.0), Hit::DrawerApp(i)));
    }

    // Scroll thumb.
    if content_h > view_h {
        let track = Rect::from_min_size(
            pos2(panel.right() - 5.0, panel.top() + 122.0),
            vec2(2.5, view_h - 26.0),
        );
        icons::rrect(p, track, 1.2, theme::with_alpha(pal.text, 0.08));
        let th = (view_h / content_h).clamp(0.15, 1.0);
        let frac = (app.drawer_scroll / (content_h - view_h)).clamp(0.0, 1.0);
        icons::rrect(
            p,
            Rect::from_min_size(
                pos2(track.left(), track.top() + frac * track.height() * (1.0 - th)),
                vec2(track.width(), track.height() * th),
            ),
            1.2,
            theme::with_alpha(pal.text, 0.35),
        );
    }

    // Hidden Space hint on the right edge.
    let hy = panel.top() + panel.height() * 0.40;
    let hx = panel.right() - 10.0;
    p.add(eframe::egui::Shape::line(
        vec![
            pos2(hx - 3.0, hy - 5.0),
            pos2(hx + 3.0, hy),
            pos2(hx - 3.0, hy + 5.0),
        ],
        Stroke::new(2.0, theme::white(0.35)),
    ));
    label(
        p,
        pos2(panel.right() - 8.0, hy + 16.0),
        Align2::RIGHT_CENTER,
        "Hidden Space",
        theme::font(11.0),
        theme::white(0.35),
        false,
    );
}

// ------------------------------------------------------------ hidden space

fn draw_hidden(
    app: &LauncherLab,
    p: &Painter,
    rect: Rect,
    hits: &mut Vec<(Rect, Hit)>,
    _pal: &Palette,
) {
    let a = app.hidden_t;
    p.rect_filled(rect, CornerRadius::ZERO, theme::black(a));

    if app.hidden.is_unlocked() {
        let y = rect.top() + 40.0;
        let white = theme::white(a);
        let dim = theme::white(0.8 * a);
        let xc = pos2(rect.left() + 28.0, y);
        icons::line(p, xc - vec2(7.0, 7.0), xc + vec2(7.0, 7.0), 2.4, white);
        icons::line(
            p,
            xc + vec2(-7.0, 7.0),
            xc + vec2(7.0, -7.0),
            2.4,
            white,
        );
        label(
            p,
            pos2(rect.left() + 56.0, y),
            Align2::LEFT_CENTER,
            "Hidden Space",
            theme::font_medium(20.0),
            white,
            false,
        );
        icons::plus(p, pos2(rect.right() - 64.0, y), 7.0, 2.4, dim);
        icons::dots3v(p, pos2(rect.right() - 30.0, y), 6.5, 2.1, dim);
        hits.push((Rect::from_center_size(xc, vec2(48.0, 48.0)), Hit::HiddenClose));
        hits.push((
            Rect::from_center_size(pos2(rect.right() - 64.0, y), vec2(44.0, 44.0)),
            Hit::HiddenAdd,
        ));
        hits.push((
            Rect::from_center_size(pos2(rect.right() - 30.0, y), vec2(44.0, 44.0)),
            Hit::HiddenMenu,
        ));

        let slot_w = rect.width() / COLS as f32;
        for (i, it) in app.hidden.items().iter().enumerate() {
            let cx = rect.left() + (i % COLS) as f32 * slot_w + slot_w * 0.5;
            let cy = rect.top() + 96.0 + (i / COLS) as f32 * 96.0 + 32.0;
            let s = Rect::from_center_size(pos2(cx, cy), vec2(56.0, 56.0));
            let letter = it
                .name
                .chars()
                .next()
                .map(|c| c.to_string())
                .unwrap_or_default();
            icons::draw_letter(p, s, it.accent, &letter);
            label(
                p,
                pos2(cx, cy + 40.0),
                Align2::CENTER_CENTER,
                &it.name,
                theme::font(13.0),
                theme::white(0.86 * a),
                false,
            );
            hits.push((s.expand(12.0), Hit::HiddenItem(i)));
        }
    } else {
        // Password gate (the original shows a text-password dialog; the
        // demo keeps the reconstructed hold-to-unlock pad).
        let c = pos2(rect.center().x, rect.top() + rect.height() * 0.40);
        icons::arc(
            p,
            c + vec2(0.0, -12.0),
            14.0,
            4.5,
            180.0,
            360.0,
            theme::white(0.9 * a),
        );
        icons::rrect(
            p,
            Rect::from_center_size(c + vec2(0.0, 12.0), vec2(46.0, 38.0)),
            9.0,
            theme::white(0.9 * a),
        );
        label(
            p,
            c + vec2(0.0, 62.0),
            Align2::CENTER_CENTER,
            "Hidden Space is protected",
            theme::font(16.0),
            theme::white(0.95 * a),
            false,
        );
        let ring_c = pos2(rect.center().x, rect.top() + rect.height() * 0.60);
        p.circle_stroke(ring_c, 26.0, Stroke::new(3.0, theme::white(0.18 * a)));
        let prog = (app.unlock_hold / 0.65).clamp(0.0, 1.0);
        if prog > 0.001 {
            icons::arc(
                p,
                ring_c,
                26.0,
                3.0,
                -90.0,
                -90.0 + 360.0 * prog,
                theme::white(0.9 * a),
            );
        }
        label(
            p,
            ring_c + vec2(0.0, 46.0),
            Align2::CENTER_CENTER,
            "hold to unlock",
            theme::font(13.0),
            theme::white(0.6 * a),
            false,
        );
        hits.push((
            Rect::from_center_size(ring_c, vec2(96.0, 96.0)),
            Hit::HiddenHold,
        ));
    }
}

// ------------------------------------------------------------- status/toast

fn draw_status(app: &LauncherLab, p: &Painter, rect: Rect) {
    let cy = rect.top() + 15.0;
    let time = clock::clock_label(app.time);
    label(
        p,
        pos2(rect.left() + 26.0, cy),
        Align2::LEFT_CENTER,
        &time,
        theme::font_medium(14.0),
        Color32::WHITE,
        true,
    );

    let right = rect.right() - 25.0;
    let (bw, bh) = (22.0, 11.0);
    let bx = right - bw;
    icons::rrect(
        p,
        Rect::from_center_size(pos2(bx + bw * 0.5, cy), vec2(bw, bh)),
        3.2,
        theme::white(0.38),
    );
    icons::rrect(
        p,
        Rect::from_min_size(
            pos2(bx + 1.6, cy - bh * 0.5 + 1.6),
            vec2((bw - 3.2) * 0.8, bh - 3.2),
        ),
        1.8,
        theme::white(0.95),
    );
    icons::rrect(
        p,
        Rect::from_center_size(pos2(right + 1.4, cy), vec2(2.4, 4.5)),
        1.2,
        theme::white(0.38),
    );
    // Wi-Fi.
    let wc = pos2(bx - 20.0, cy + 5.0);
    icons::arc(p, wc, 7.6, 1.8, 205.0, 335.0, Color32::WHITE);
    icons::arc(p, wc, 4.6, 1.8, 205.0, 335.0, Color32::WHITE);
    p.circle_filled(wc, 1.2, Color32::WHITE);
    // Signal wedge (OxygenOS style).
    let sx = wc.x - 26.5;
    p.add(eframe::egui::Shape::convex_polygon(
        vec![
            pos2(sx, cy + 6.5),
            pos2(sx + 13.0, cy + 6.5),
            pos2(sx + 13.0, cy - 6.5),
        ],
        Color32::WHITE,
        Stroke::NONE,
    ));
}

fn draw_toast(p: &Painter, rect: Rect, msg: &str, ttl: f32, pal: &Palette) {
    let a = (ttl / 0.4).clamp(0.0, 1.0);
    let galley = p.layout_no_wrap(msg.to_owned(), theme::font(13.0), Color32::WHITE);
    let pill = Rect::from_center_size(
        pos2(rect.center().x, rect.bottom() - 96.0),
        galley.size() + vec2(36.0, 22.0),
    );
    icons::rrect(
        p,
        pill,
        pill.height() * 0.5,
        theme::with_alpha(pal.toast_bg, 0.94 * a),
    );
    p.galley(pill.center() - galley.size() * 0.5, galley, Color32::WHITE);
}

// ------------------------------------------------------------------ locked

fn draw_locked(app: &LauncherLab, p: &Painter, rect: Rect, hits: &mut Vec<(Rect, Hit)>) {
    let a = app.lock_t;
    p.rect_filled(rect, CornerRadius::ZERO, theme::black(0.92 * a));
    let cx = rect.center().x;
    label(
        p,
        pos2(cx, rect.top() + 210.0),
        Align2::CENTER_CENTER,
        &clock::clock_label(app.time),
        theme::font_light(64.0),
        theme::white(0.95 * a),
        false,
    );
    label(
        p,
        pos2(cx, rect.top() + 264.0),
        Align2::CENTER_CENTER,
        &clock::date_label(app.time),
        theme::font(15.0),
        theme::white(0.7 * a),
        false,
    );
    let lc = pos2(cx, rect.top() + rect.height() * 0.46);
    icons::arc(
        p,
        lc + vec2(0.0, -14.0),
        14.0,
        5.0,
        180.0,
        360.0,
        theme::white(0.9 * a),
    );
    icons::rrect(
        p,
        Rect::from_center_size(lc + vec2(0.0, 10.0), vec2(46.0, 38.0)),
        9.0,
        theme::white(0.9 * a),
    );
    label(
        p,
        pos2(cx, rect.bottom() - 150.0),
        Align2::CENTER_CENTER,
        "Tap to unlock",
        theme::font(14.0),
        theme::white(0.65 * a),
        false,
    );
    hits.push((rect, Hit::UnlockTap));
}

// ------------------------------------------------------------------- shelf

fn draw_shelf(
    app: &LauncherLab,
    p: &Painter,
    rect: Rect,
    hits: &mut Vec<(Rect, Hit)>,
    pal: &Palette,
) -> f32 {
    let panel = rect.translate(vec2(-(1.0 - app.shelf_t) * rect.width(), 0.0));
    p.rect_filled(panel, CornerRadius::ZERO, pal.panel);

    let left = panel.left() + 20.0;
    let right = panel.right() - 20.0;
    let w = right - left;
    let slot = w / 5.0;
    let mut y = panel.top() + 52.0 - app.shelf_scroll;

    // --- Toolbox -------------------------------------------------------
    let hc = pos2(left + 14.0, y + 14.0);
    icons::rr(p, hc, 28.0, 28.0, 8.0, theme::rgb(0xFF7A1A));
    glyph_toolbox(p, hc, 28.0, Color32::WHITE);
    label(
        p,
        pos2(left + 44.0, y + 14.0),
        Align2::LEFT_CENTER,
        "Toolbox",
        theme::font_medium(20.0),
        pal.text,
        false,
    );
    glyph_pencil(p, pos2(right - 12.0, y + 14.0), 22.0, pal.text_dim);
    y += 44.0;

    const TOOL_NAMES: [&str; 5] = ["Calculator", "Selfie", "New timer", "Send files", "New tab"];
    for i in 0..5 {
        let cx = left + (i as f32 + 0.5) * slot;
        let cy = y + 28.0;
        let s = Rect::from_center_size(pos2(cx, cy), vec2(52.0, 52.0));
        if i == 0 {
            App::Calculator.draw(p, s);
        } else {
            icons::disc(p, s.center(), 26.0, theme::rgb(0xF2F2F4));
            match i {
                1 => glyph_person(p, s.center(), 26.0, theme::rgb(0x4C5560)),
                2 => glyph_hourglass(p, s.center(), 24.0, theme::rgb(0x7C4DFF)),
                3 => glyph_send(p, s.center(), 24.0, theme::rgb(0x4285F4)),
                _ => {
                    icons::plus(p, s.center(), 9.0, 3.0, theme::rgb(0x4285F4));
                    icons::disc(p, s.center() + vec2(11.0, 10.0), 4.0, theme::rgb(0xEA4335));
                }
            }
        }
        label(
            p,
            pos2(cx, cy + 40.0),
            Align2::CENTER_CENTER,
            TOOL_NAMES[i],
            theme::font(12.0),
            theme::with_alpha(pal.text, 0.88),
            false,
        );
        hits.push((s.expand(10.0), Hit::ShelfTool(i)));
    }
    y += 96.0 + 26.0;

    // --- Recent apps ---------------------------------------------------
    let hc = pos2(left + 14.0, y + 14.0);
    icons::disc(p, hc, 14.0, theme::rgb(0x4285F4));
    for dx in [-5.0f32, 5.0] {
        for dy in [-5.0f32, 5.0] {
            icons::disc(p, hc + vec2(dx, dy), 2.0, Color32::WHITE);
        }
    }
    label(
        p,
        pos2(left + 44.0, y + 14.0),
        Align2::LEFT_CENTER,
        "Recent apps",
        theme::font_medium(20.0),
        pal.text,
        false,
    );
    y += 44.0;
    let row_h = 78.0;
    for (i, a) in icons::SHELF_RECENTS.iter().enumerate() {
        let cx = left + ((i % 5) as f32 + 0.5) * slot;
        let cy = y + (i / 5) as f32 * row_h + 24.0;
        let s = Rect::from_center_size(pos2(cx, cy), vec2(48.0, 48.0));
        a.draw(p, s);
        label(
            p,
            pos2(cx, cy + 34.0),
            Align2::CENTER_CENTER,
            a.name(),
            theme::font(12.0),
            theme::with_alpha(pal.text, 0.85),
            false,
        );
        hits.push((s.expand(10.0), Hit::ShelfApp(i)));
    }
    y += 2.0 * row_h + 6.0 + 26.0;

    // --- Parking Location ----------------------------------------------
    let hc = pos2(left + 14.0, y + 14.0);
    icons::disc(p, hc, 14.0, theme::rgb(0x8A63F4));
    p.text(hc, Align2::CENTER_CENTER, "P", theme::font_medium(15.0), Color32::WHITE);
    label(
        p,
        pos2(left + 44.0, y + 14.0),
        Align2::LEFT_CENTER,
        "Parking Location",
        theme::font_medium(20.0),
        pal.text,
        false,
    );
    label(
        p,
        pos2(right - 4.0, y + 14.0),
        Align2::RIGHT_CENTER,
        "FINISH",
        theme::font_medium(13.0),
        pal.accent,
        false,
    );
    y += 56.0;
    label(
        p,
        pos2(left, y),
        Align2::LEFT_TOP,
        "Parking duration: 0 minutes",
        theme::font(16.0),
        pal.text,
        false,
    );
    y += 34.0;
    let map = Rect::from_min_size(pos2(left, y), vec2(w, 150.0));
    icons::rrect(p, map, 12.0, theme::rgb(0x26262B));
    let road = theme::white(0.10);
    icons::line(
        p,
        pos2(map.left() + 20.0, map.top() + 56.0),
        pos2(map.right() - 20.0, map.top() + 44.0),
        10.0,
        road,
    );
    icons::line(
        p,
        pos2(map.left() + 20.0, map.top() + 110.0),
        pos2(map.right() - 20.0, map.top() + 120.0),
        8.0,
        road,
    );
    icons::line(
        p,
        pos2(map.center().x - 30.0, map.top() + 10.0),
        pos2(map.center().x + 6.0, map.bottom() - 10.0),
        8.0,
        road,
    );
    let pin = pos2(map.center().x + 6.0, map.center().y);
    icons::poly(
        p,
        &[pin + vec2(-9.0, -2.0), pin + vec2(9.0, -2.0), pin + vec2(0.0, 16.0)],
        theme::rgb(0x2196F3),
    );
    icons::disc(p, pin, 10.0, theme::rgb(0x2196F3));
    icons::disc(p, pin, 4.0, Color32::WHITE);
    y += 150.0 + 12.0;

    let c1 = left + w * 0.25;
    let c2 = left + w * 0.75;
    let dim = pal.text_dim;
    glyph_camera_small(p, pos2(c1 - 52.0, y + 24.0), 20.0, dim);
    label(p, pos2(c1 - 34.0, y + 24.0), Align2::LEFT_CENTER, "Take pictures", theme::font(15.0), dim, false);
    glyph_nav(p, pos2(c2 - 40.0, y + 24.0), 20.0, dim);
    label(p, pos2(c2 - 22.0, y + 24.0), Align2::LEFT_CENTER, "Navigation", theme::font(15.0), dim, false);
    hits.push((Rect::from_min_max(pos2(left, y), pos2(right, y + 48.0)), Hit::ShelfAction));
    y += 48.0 + 26.0;

    // --- CardProvider demo cards ---------------------------------------
    for (i, card) in app.cards.cards().iter().take(3).enumerate() {
        let hc = pos2(left + 14.0, y + 14.0);
        icons::disc(p, hc, 14.0, theme::with_alpha(theme::rgb(card.accent), 0.95));
        let letter = card
            .channel
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_default();
        p.text(hc, Align2::CENTER_CENTER, &letter, theme::font_medium(15.0), Color32::WHITE);
        label(p, pos2(left + 44.0, y + 14.0), Align2::LEFT_CENTER, &card.title, theme::font_medium(20.0), pal.text, false);
        y += 44.0;
        let body_col = theme::with_alpha(pal.text, 0.75);
        let body = p.layout(card.body.clone(), theme::font(15.0), body_col, w);
        let bh = body.size().y;
        p.galley(pos2(left, y), body, body_col);
        y += bh + 6.0;
        label(
            p,
            pos2(left, y),
            Align2::LEFT_TOP,
            &format!("{} \u{00B7} {}", card.channel, card.tag),
            theme::font(11.0),
            pal.text_faint,
            false,
        );
        hits.push((
            Rect::from_min_max(pos2(left - 8.0, y - bh - 52.0), pos2(right, y + 22.0)),
            Hit::ShelfCard(i),
        ));
        y += 22.0 + 26.0;
    }

    // --- New note ------------------------------------------------------
    icons::plus(p, pos2(left + 9.0, y + 10.0), 7.0, 2.4, pal.accent);
    label(p, pos2(left + 26.0, y + 10.0), Align2::LEFT_CENTER, "New note", theme::font_medium(15.0), pal.accent, false);
    hits.push((Rect::from_min_max(pos2(left - 8.0, y - 12.0), pos2(right, y + 32.0)), Hit::ShelfNewNote));
    y += 48.0;

    label(
        p,
        pos2(panel.center().x, y),
        Align2::CENTER_CENTER,
        "CardProvider: tap = getCard \u{00B7} hold a memo = deleteCard",
        theme::font(11.0),
        pal.text_faint,
        false,
    );
    y += 30.0;

    (y + app.shelf_scroll) - panel.top()
}

// ----------------------------------------------------------------- recents

fn draw_recents(
    app: &LauncherLab,
    p: &Painter,
    rect: Rect,
    hits: &mut Vec<(Rect, Hit)>,
    pal: &Palette,
) {
    let a = app.overview_t;
    p.rect_filled(rect, CornerRadius::ZERO, theme::black(0.62 * a));
    if app.tasks.is_empty() {
        label(
            p,
            rect.center(),
            Align2::CENTER_CENTER,
            "No recent items",
            theme::font(16.0),
            theme::white(0.5 * a),
            false,
        );
        return;
    }
    let card_w = (rect.width() - 64.0).max(120.0);
    let card_top = rect.top() + 94.0;
    let card_h = (rect.bottom() - 240.0 - card_top).max(140.0);
    let pitch = card_w + 24.0;
    let carousel = app.carousel.min(app.tasks.len() - 1);
    let mut bottom_icons: Vec<(usize, App)> = Vec::new();

    for (i, task) in app.tasks.iter().enumerate() {
        let bx = rect.center().x - card_w * 0.5 + (i as f32 - carousel as f32) * pitch
            + app.carousel_drag;
        let mut card = Rect::from_min_size(pos2(bx, card_top), vec2(card_w, card_h));
        if card.right() < rect.left() - 40.0 || card.left() > rect.right() + 40.0 {
            continue;
        }
        if let Some((ti, _, dy)) = app.task_drag {
            if ti == i {
                card = card.translate(vec2(0.0, dy));
            }
        }
        let fade = if task.dying {
            (1.0 - task.dismiss_t).clamp(0.0, 1.0)
        } else {
            1.0
        };

        icons::rrect(
            p,
            card.translate(vec2(0.0, 3.0)).expand(2.0),
            18.0,
            theme::black(0.30 * fade * a),
        );
        icons::rrect(p, card, 16.0, theme::with_alpha(pal.card, fade));

        // Header above the card: icon + name, centered.
        let tw = p
            .layout_no_wrap(task.title.clone(), theme::font_medium(15.0), Color32::WHITE)
            .size()
            .x;
        let x0 = card.center().x - (35.0 + 8.0 + tw) * 0.5;
        let hdr = pos2(x0 + 17.5, card.top() - 26.0);
        task.app.draw(p, Rect::from_center_size(hdr, vec2(35.0, 35.0)));
        label(
            p,
            pos2(x0 + 43.0, hdr.y),
            Align2::LEFT_CENTER,
            &task.title,
            theme::font_medium(15.0),
            theme::white(0.95 * a * fade),
            false,
        );

        // Fake app screenshot.
        let cp = p.with_clip_rect(card);
        icons::rrect(
            &cp,
            Rect::from_min_size(card.min, vec2(card_w, 30.0)),
            0.0,
            theme::with_alpha(theme::rgb(task.accent), 0.25 * fade),
        );
        task.app
            .draw(&cp, Rect::from_center_size(pos2(card.left() + 20.0, card.top() + 15.0), vec2(20.0, 20.0)));
        icons::dots3h(&cp, pos2(card.right() - 34.0, card.top() + 15.0), 8.0, 2.0, theme::white(0.3 * fade));
        let line = |yy: f32, wf: f32, h: f32| {
            Rect::from_min_size(
                pos2(card.left() + 18.0, card.top() + yy),
                vec2((card_w - 36.0) * wf, h),
            )
        };
        icons::rrect(&cp, line(58.0, 0.55, 14.0), 4.0, theme::white(0.12 * fade));
        icons::rrect(&cp, line(86.0, 0.75, 8.0), 4.0, theme::white(0.08 * fade));
        icons::rrect(&cp, line(104.0, 0.68, 8.0), 4.0, theme::white(0.08 * fade));
        icons::rrect(&cp, line(122.0, 0.60, 8.0), 4.0, theme::white(0.08 * fade));
        icons::rrect(
            &cp,
            Rect::from_min_size(
                pos2(card.left() + 18.0, card.top() + 150.0),
                vec2(card_w - 36.0, (card_h - 240.0).max(40.0)),
            ),
            8.0,
            theme::white(0.05 * fade),
        );
        icons::rrect(
            &cp,
            Rect::from_center_size(pos2(card.center().x, card.bottom() - 22.0), vec2(card_w * 0.4, 6.0)),
            3.0,
            theme::white(0.10 * fade),
        );

        if fade > 0.4 {
            hits.push((card, Hit::Task(i)));
        }
        if bottom_icons.len() < 6 {
            bottom_icons.push((i, task.app));
        }
    }

    let n = bottom_icons.len();
    if n > 1 {
        let dots_y = rect.bottom() - 176.0;
        let total = n as f32 * 3.5 + (n - 1) as f32 * 7.0;
        let x0 = rect.center().x - total * 0.5;
        for k in 0..n {
            let active = bottom_icons[k].0 == carousel;
            icons::rrect(
                p,
                Rect::from_min_size(pos2(x0 + k as f32 * 10.5, dots_y - 1.75), vec2(3.5, 3.5)),
                1.0,
                theme::white(if active { 0.9 } else { 0.25 }),
            );
        }
    }
    if n > 0 {
        let spacing = 56.0f32.min((rect.width() - 80.0) / n as f32);
        for (k, (ti, app)) in bottom_icons.iter().enumerate() {
            let cx = rect.center().x + (k as f32 - (n as f32 - 1.0) * 0.5) * spacing;
            let s = Rect::from_center_size(pos2(cx, rect.bottom() - 152.0), vec2(35.0, 35.0));
            app.draw(p, s);
            hits.push((s.expand(8.0), Hit::TaskIcon(*ti)));
        }
    }
    let pill = Rect::from_center_size(pos2(rect.center().x, rect.bottom() - 91.0), vec2(132.0, 38.0));
    icons::rrect(p, pill, 19.0, theme::white(0.14));
    label(p, pill.center(), Align2::CENTER_CENTER, "Clear all", theme::font_medium(14.0), theme::white(0.9), false);
    hits.push((pill, Hit::TaskClearAll));
}

// ------------------------------------------------------------ shelf glyphs

fn glyph_toolbox(p: &Painter, c: Pos2, s: f32, col: Color32) {
    let u = |x: f32, y: f32| c + vec2(x * s, y * s);
    icons::rr(p, u(0.0, 0.06), s * 0.62, s * 0.40, s * 0.08, col);
    icons::rr(p, u(0.0, -0.18), s * 0.70, s * 0.14, s * 0.05, col);
    icons::arc(p, u(0.0, -0.20), s * 0.16, s * 0.07, 180.0, 360.0, col);
}

fn glyph_pencil(p: &Painter, c: Pos2, s: f32, col: Color32) {
    let u = |x: f32, y: f32| c + vec2(x * s, y * s);
    icons::poly(
        p,
        &[u(-0.30, 0.24), u(0.16, -0.36), u(0.34, -0.20), u(-0.14, 0.38)],
        col,
    );
    icons::poly(p, &[u(-0.14, 0.38), u(-0.44, 0.46), u(-0.30, 0.24)], col);
}

fn glyph_person(p: &Painter, c: Pos2, s: f32, col: Color32) {
    icons::disc(p, c + vec2(0.0, -s * 0.22), s * 0.22, col);
    icons::rr(p, c + vec2(0.0, s * 0.26), s * 0.62, s * 0.34, s * 0.16, col);
}

fn glyph_hourglass(p: &Painter, c: Pos2, s: f32, col: Color32) {
    let u = |x: f32, y: f32| c + vec2(x * s, y * s);
    icons::poly(p, &[u(-0.40, -0.50), u(0.40, -0.50), u(0.0, 0.0)], col);
    icons::poly(p, &[u(-0.40, 0.50), u(0.40, 0.50), u(0.0, 0.0)], col);
    icons::line(p, u(-0.48, -0.52), u(0.48, -0.52), s * 0.10, col);
    icons::line(p, u(-0.48, 0.52), u(0.48, 0.52), s * 0.10, col);
}

fn glyph_send(p: &Painter, c: Pos2, s: f32, col: Color32) {
    let u = |x: f32, y: f32| c + vec2(x * s, y * s);
    icons::line(p, u(0.0, 0.42), u(0.0, -0.08), s * 0.15, col);
    icons::poly(p, &[u(-0.30, -0.04), u(0.30, -0.04), u(0.0, -0.52)], col);
    icons::line(p, u(-0.46, 0.46), u(0.46, 0.46), s * 0.13, col);
}

fn glyph_camera_small(p: &Painter, c: Pos2, s: f32, col: Color32) {
    icons::rr(p, c, s * 0.68, s * 0.50, s * 0.12, col);
    icons::rr(p, c + vec2(0.0, -s * 0.34), s * 0.28, s * 0.12, s * 0.05, col);
    p.circle_stroke(c, s * 0.13, Stroke::new(s * 0.09, col));
}

fn glyph_nav(p: &Painter, c: Pos2, s: f32, col: Color32) {
    let u = |x: f32, y: f32| c + vec2(x * s, y * s);
    icons::poly(
        p,
        &[u(-0.45, 0.50), u(0.0, -0.55), u(0.45, 0.50), u(0.0, 0.24)],
        col,
    );
}

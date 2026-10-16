//! All rendering. Painter-only: every control is a hand-drawn primitive plus
//! a hit rectangle that input routing in `crate::app` can pick up.
//!
//! The geometry reconstructs the values decoded from the original APK
//! (`res/values/dimens.xml`, `res/xml/device_profiles.xml`,
//! `default_workspace_5x5_7.xml`): 5 columns x 6 rows of 64 dp cells, 15 dp
//! side margins, 18 dp top padding, 66 dp hotseat, status bar drawn over the
//! wallpaper, "^" caret + square dots above the dock.

use eframe::egui::{
    pos2, vec2, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, StrokeKind,
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
    /// In-app demo preview: back chevron.
    AppBack,
    /// In-app demo preview: mock action button `i`.
    AppAction(usize),
    /// Notification shade.
    ShadeTile(usize),
    ShadeHandle,
    ShadeClose,
    ShadeBrightness,
    ShadeClearAll,
    ShadeNotif(usize),
    /// First-run gesture guide button.
    OnboardOk,
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

    wallpaper::paint(p, rect, app.pager.offset);

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
    if app.screen == Screen::Shade || app.shade_t > 0.001 {
        draw_shade(app, p, rect, &mut hits, &pal);
    }
    if app.open_app.is_some() {
        draw_app_mock(app, p, rect, &mut hits, &pal);
    }
    if app.lock_t > 0.001 {
        draw_locked(app, p, rect, &mut hits);
    }

    // Brightness dim driven by the shade slider.
    let dim = ((1.0 - app.brightness) * 0.55).clamp(0.0, 0.6);
    if dim > 0.002 {
        p.rect_filled(rect, CornerRadius::ZERO, theme::black(dim));
    }
    if app.onboard && app.screen == Screen::Home {
        draw_onboarding(p, rect, &mut hits, &pal);
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
        Stroke::new(2.2_f32, theme::white(0.82)),
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
    p.circle_stroke(lc, 7.5, Stroke::new(1.6_f32, pal.lens));
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
    let visible = app.visible_drawer();
    let rows = visible.len().div_ceil(COLS);
    let content_h = rows as f32 * row_h + 40.0;
    let view_h = (panel.height() - 118.0).max(1.0);
    if visible.is_empty() {
        label(
            p,
            pos2(panel.center().x, panel.top() + 240.0),
            Align2::CENTER_CENTER,
            "All apps live in Hidden Space",
            theme::font(15.0),
            pal.text_dim,
            false,
        );
    }
    for (i, a) in visible.iter().enumerate() {
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
        Stroke::new(2.0_f32, theme::white(0.35)),
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

        // Real hidden apps first (their indices match `hidden_apps`), then
        // the demo items of the reconstructed model.
        let mut entries: Vec<(Option<App>, String, u32)> = Vec::new();
        for ha in &app.hidden_apps {
            entries.push((Some(*ha), ha.name().to_string(), ha.accent()));
        }
        for it in app.hidden.items() {
            entries.push((None, it.name.clone(), it.accent));
        }
        if entries.is_empty() {
            label(
                p,
                pos2(rect.center().x, rect.top() + 240.0),
                Align2::CENTER_CENTER,
                "No hidden apps",
                theme::font(16.0),
                theme::white(0.7 * a),
                false,
            );
            label(
                p,
                pos2(rect.center().x, rect.top() + 268.0),
                Align2::CENTER_CENTER,
                "Hold an app in the drawer to hide it",
                theme::font(13.0),
                theme::white(0.45 * a),
                false,
            );
        }
        let slot_w = rect.width() / COLS as f32;
        for (i, (oa, name, accent)) in entries.iter().enumerate() {
            let cx = rect.left() + (i % COLS) as f32 * slot_w + slot_w * 0.5;
            let cy = rect.top() + 96.0 + (i / COLS) as f32 * 96.0 + 32.0;
            let s = Rect::from_center_size(pos2(cx, cy), vec2(56.0, 56.0));
            match oa {
                Some(ha) => ha.draw(p, s),
                None => {
                    let letter = name
                        .chars()
                        .next()
                        .map(|c| c.to_string())
                        .unwrap_or_default();
                    icons::draw_letter(p, s, *accent, &letter);
                }
            }
            label(
                p,
                pos2(cx, cy + 40.0),
                Align2::CENTER_CENTER,
                name,
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
        p.circle_stroke(ring_c, 26.0, Stroke::new(3.0_f32, theme::white(0.18 * a)));
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
    p.rect_filled(rect, CornerRadius::ZERO, theme::black(0.94 * a));
    let cx = rect.center().x;
    label(
        p,
        pos2(cx, rect.top() + 190.0),
        Align2::CENTER_CENTER,
        &clock::clock_label(app.time),
        theme::font_light(60.0),
        theme::white(0.95 * a),
        false,
    );
    label(
        p,
        pos2(cx, rect.top() + 238.0),
        Align2::CENTER_CENTER,
        &clock::date_label(app.time),
        theme::font(15.0),
        theme::white(0.7 * a),
        false,
    );
    if !app.notifs.is_empty() {
        label(
            p,
            pos2(cx, rect.top() + 266.0),
            Align2::CENTER_CENTER,
            &format!("{} notifications", app.notifs.len()),
            theme::font(12.0),
            theme::white(0.45 * a),
            false,
        );
    }
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
        "Swipe up or tap to unlock",
        theme::font(14.0),
        theme::white(0.65 * a),
        false,
    );
    // Bottom shortcuts (decor): flashlight / camera.
    let fl = pos2(rect.left() + 52.0, rect.bottom() - 64.0);
    p.circle_filled(fl, 22.0, theme::white(0.10 * a));
    icons::glyph_flashlight(p, fl, 20.0, theme::white(0.8 * a));
    let cm = pos2(rect.right() - 52.0, rect.bottom() - 64.0);
    p.circle_filled(cm, 22.0, theme::white(0.10 * a));
    icons::glyph_shutter(p, cm, 9.0, theme::white(0.8 * a));
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
        p.rect_stroke(
            card,
            CornerRadius::same(16),
            Stroke::new(1.0_f32, theme::with_alpha(pal.card_line, fade)),
            StrokeKind::Inside,
        );

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

// ------------------------------------------------------------------- shade

/// Brightness slider band inside the shade panel (panel-top-relative). The
/// drag handling in `app.rs` uses the same constants.
pub(crate) const SHADE_BRIGHT_TOP: f32 = 246.0;
pub(crate) const SHADE_BRIGHT_BOTTOM: f32 = 330.0;
/// Left/right insets of the brightness slider inside the panel.
pub(crate) const SHADE_SLIDER_LEFT: f32 = 40.0;
pub(crate) const SHADE_SLIDER_RIGHT: f32 = 40.0;

fn draw_shade(
    app: &LauncherLab,
    p: &Painter,
    rect: Rect,
    hits: &mut Vec<(Rect, Hit)>,
    pal: &Palette,
) {
    let a = app.shade_t.clamp(0.0, 1.0);
    let ph = (rect.height() * 0.72).clamp(500.0, 700.0);
    let y0 = rect.top() - (1.0 - a) * ph;
    let panel = Rect::from_min_max(pos2(rect.left(), y0 - 30.0), pos2(rect.right(), y0 + ph));

    // Scrim below the panel (tap to close).
    let below = Rect::from_min_max(pos2(rect.left(), y0 + ph), rect.max);
    if below.height() > 1.0 {
        p.rect_filled(below, CornerRadius::ZERO, theme::black(0.45 * a));
        hits.push((below, Hit::ShadeClose));
    }

    icons::rrect(p, panel, 28.0, theme::with_alpha(pal.panel, 0.98 * a));

    // Drag handle.
    icons::rrect(
        p,
        Rect::from_center_size(pos2(panel.center().x, y0 + 14.0), vec2(38.0, 4.5)),
        2.2,
        theme::white(0.35 * a),
    );
    hits.push((
        Rect::from_center_size(pos2(panel.center().x, y0 + 16.0), vec2(120.0, 44.0)),
        Hit::ShadeHandle,
    ));

    // Clock + date.
    label(
        p,
        pos2(panel.left() + 28.0, y0 + 56.0),
        Align2::LEFT_CENTER,
        &clock::clock_label(app.time),
        theme::font_light(40.0),
        theme::white(0.95 * a),
        false,
    );
    label(
        p,
        pos2(panel.left() + 29.0, y0 + 86.0),
        Align2::LEFT_CENTER,
        &clock::date_label(app.time),
        theme::font(13.0),
        theme::white(0.6 * a),
        false,
    );
    label(
        p,
        pos2(panel.right() - 28.0, y0 + 62.0),
        Align2::RIGHT_CENTER,
        "OxygenOS 10 shade",
        theme::font(11.0),
        theme::white(0.4 * a),
        false,
    );

    // Quick tiles: 3 x 2.
    const TILES: [&str; 6] = [
        "Wi-Fi",
        "Bluetooth",
        "Flashlight",
        "Airplane",
        "Auto-rotate",
        "Battery saver",
    ];
    let gx = panel.left() + 16.0;
    let gw = (panel.width() - 32.0 - 24.0) / 3.0;
    let th = 54.0;
    for i in 0..6 {
        let col = i % 3;
        let row = i / 3;
        let tr = Rect::from_min_size(
            pos2(gx + col as f32 * (gw + 12.0), y0 + 116.0 + row as f32 * (th + 12.0)),
            vec2(gw, th),
        );
        let on = app.tiles.get(i).copied().unwrap_or(false);
        let bg = if on {
            theme::with_alpha(pal.accent, 0.90 * a)
        } else {
            theme::white(0.10 * a)
        };
        icons::rrect(p, tr, 16.0, bg);
        let ic = pos2(tr.left() + 21.0, tr.center().y);
        let gc = if on { Color32::WHITE } else { theme::white(0.72 * a) };
        match i {
            0 => icons::glyph_wifi(p, ic, 19.0, gc),
            1 => icons::glyph_bluetooth(p, ic, 19.0, gc),
            2 => icons::glyph_flashlight(p, ic, 19.0, gc),
            3 => icons::glyph_airplane(p, ic, 19.0, gc),
            4 => icons::glyph_rotate(p, ic, 19.0, gc),
            _ => icons::glyph_bolt(p, ic, 19.0, gc),
        }
        label(
            p,
            pos2(tr.left() + 42.0, tr.center().y),
            Align2::LEFT_CENTER,
            TILES[i],
            theme::font(12.5),
            if on { Color32::WHITE } else { theme::white(0.85 * a) },
            false,
        );
        hits.push((tr, Hit::ShadeTile(i)));
    }

    // Brightness (drag to change; the app dims the whole surface).
    label(
        p,
        pos2(gx, y0 + SHADE_BRIGHT_TOP + 8.0),
        Align2::LEFT_CENTER,
        "Brightness",
        theme::font(12.0),
        theme::white(0.55 * a),
        false,
    );
    let pct = (app.brightness * 100.0).round() as i32;
    label(
        p,
        pos2(panel.right() - 16.0, y0 + SHADE_BRIGHT_TOP + 8.0),
        Align2::RIGHT_CENTER,
        &format!("{pct}%"),
        theme::font(12.0),
        theme::white(0.55 * a),
        false,
    );
    let sl = Rect::from_min_max(
        pos2(panel.left() + SHADE_SLIDER_LEFT, y0 + SHADE_BRIGHT_TOP + 34.0),
        pos2(panel.right() - SHADE_SLIDER_RIGHT, y0 + SHADE_BRIGHT_TOP + 46.0),
    );
    icons::rrect(p, sl, 6.0, theme::white(0.16 * a));
    let f = ((app.brightness - 0.12) / 0.88).clamp(0.0, 1.0);
    let fx = sl.left() + sl.width() * f;
    icons::rrect(
        p,
        Rect::from_min_max(sl.min, pos2(fx, sl.max.y)),
        6.0,
        theme::with_alpha(pal.accent, 0.9 * a),
    );
    p.circle_filled(pos2(fx, sl.center().y), 9.0, theme::white(0.95 * a));
    hits.push((
        Rect::from_min_max(
            pos2(sl.left() - 10.0, y0 + SHADE_BRIGHT_TOP + 20.0),
            pos2(sl.right() + 10.0, y0 + SHADE_BRIGHT_TOP + 60.0),
        ),
        Hit::ShadeBrightness,
    ));

    // Hairline between the quick controls and the notification list.
    bar(
        p,
        panel.left() + 12.0,
        y0 + SHADE_BRIGHT_BOTTOM + 4.0,
        panel.width() - 24.0,
        1.0,
        theme::with_alpha(pal.divider, a),
    );

    // Notifications.
    let mut ny = y0 + SHADE_BRIGHT_BOTTOM + 26.0;
    label(
        p,
        pos2(gx, ny),
        Align2::LEFT_CENTER,
        "Notifications",
        theme::font_medium(13.5),
        theme::white(0.9 * a),
        false,
    );
    let clr = Rect::from_center_size(pos2(panel.right() - 52.0, ny), vec2(66.0, 26.0));
    icons::rrect(p, clr, 13.0, theme::white(0.08 * a));
    label(
        p,
        clr.center(),
        Align2::CENTER_CENTER,
        "Clear all",
        theme::font(11.0),
        theme::white(0.7 * a),
        false,
    );
    hits.push((clr, Hit::ShadeClearAll));
    ny += 24.0;
    if app.notifs.is_empty() {
        label(
            p,
            pos2(panel.center().x, ny + 44.0),
            Align2::CENTER_CENTER,
            "No notifications",
            theme::font(13.0),
            theme::white(0.4 * a),
            false,
        );
    }
    for (i, n) in app.notifs.iter().take(3).enumerate() {
        let nr = Rect::from_min_size(
            pos2(panel.left() + 16.0, ny),
            vec2(panel.width() - 32.0, 64.0),
        );
        icons::rrect(p, nr, 16.0, theme::white(0.06 * a));
        n.app.draw(
            p,
            Rect::from_center_size(pos2(nr.left() + 30.0, nr.center().y), vec2(34.0, 34.0)),
        );
        label(
            p,
            pos2(nr.left() + 56.0, nr.top() + 21.0),
            Align2::LEFT_CENTER,
            &n.title,
            theme::font_medium(13.0),
            theme::white(0.95 * a),
            false,
        );
        label(
            p,
            pos2(nr.right() - 14.0, nr.top() + 21.0),
            Align2::RIGHT_CENTER,
            &n.when,
            theme::font(11.0),
            theme::white(0.45 * a),
            false,
        );
        label(
            p,
            pos2(nr.left() + 56.0, nr.top() + 42.0),
            Align2::LEFT_CENTER,
            &n.body,
            theme::font(12.0),
            theme::white(0.6 * a),
            false,
        );
        hits.push((nr, Hit::ShadeNotif(i)));
        ny += 72.0;
    }
}

// -------------------------------------------------------------- app preview

fn ease_out(t: f32) -> f32 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u
}

fn bar(p: &Painter, x: f32, y: f32, w: f32, h: f32, col: Color32) {
    icons::rrect(p, Rect::from_min_size(pos2(x, y), vec2(w, h)), h * 0.5, col);
}

fn draw_app_mock(
    app: &LauncherLab,
    p: &Painter,
    rect: Rect,
    hits: &mut Vec<(Rect, Hit)>,
    _pal: &Palette,
) {
    let Some(o) = &app.open_app else {
        return;
    };
    let e = ease_out(o.t);
    let src = o.src.intersect(rect);
    let clip = Rect::from_min_max(
        pos2(
            src.left() + (rect.left() - src.left()) * e,
            src.top() + (rect.top() - src.top()) * e,
        ),
        pos2(
            src.right() + (rect.right() - src.right()) * e,
            src.bottom() + (rect.bottom() - src.bottom()) * e,
        ),
    );
    let cp = p.with_clip_rect(clip);
    let accent = theme::rgb(o.app.accent());

    icons::rrect(&cp, rect, 0.0, theme::rgb(0x0F1115));
    icons::rrect(
        &cp,
        Rect::from_min_size(rect.min, vec2(rect.width(), rect.height() * 0.30)),
        0.0,
        theme::with_alpha(accent, 0.18),
    );

    // Header.
    let top = rect.top() + 30.0;
    let header = Rect::from_min_size(pos2(rect.left(), top), vec2(rect.width(), 58.0));
    cp.rect_filled(header, CornerRadius::ZERO, theme::black(0.22));
    icons::glyph_back(&cp, pos2(rect.left() + 28.0, top + 29.0), 20.0, theme::white(0.9));
    label(
        &cp,
        pos2(rect.center().x, top + 29.0),
        Align2::CENTER_CENTER,
        o.app.name(),
        theme::font_medium(17.0),
        Color32::WHITE,
        false,
    );
    icons::dots3v(&cp, pos2(rect.right() - 30.0, top + 29.0), 6.5, 2.1, theme::white(0.8));
    hits.push((
        Rect::from_min_size(pos2(rect.left(), top), vec2(62.0, 58.0)),
        Hit::AppBack,
    ));
    hits.push((
        Rect::from_min_size(pos2(rect.right() - 62.0, top), vec2(62.0, 58.0)),
        Hit::AppAction(0),
    ));

    let c = Rect::from_min_max(
        pos2(rect.left(), header.bottom()),
        pos2(rect.right(), rect.bottom() - 26.0),
    );
    match o.app {
        App::Chrome | App::Google | App::Duo | App::GooglePay | App::Gmail => {
            browser_mock(&cp, c, accent);
        }
        App::Messages | App::Forums => chat_mock(&cp, c, accent, o.app),
        App::Camera => camera_mock(&cp, c, hits),
        App::Gallery | App::Photos => gallery_mock(&cp, c),
        App::PlayStore | App::PlayMovies => store_mock(&cp, c, hits),
        App::Music | App::PlayMusic | App::Recorder => music_mock(&cp, c, accent, hits),
        App::Weather => weather_mock(&cp, c, accent, hits),
        App::Clock => clock_mock(&cp, c, app),
        App::Calculator => calc_mock(&cp, c, hits),
        App::Maps => maps_mock(&cp, c, accent),
        App::YouTube | App::Netflix => video_mock(&cp, c, accent),
        _ => skeleton_mock(&cp, c, accent),
    }

    label(
        &cp,
        pos2(rect.center().x, rect.bottom() - 13.0),
        Align2::CENTER_CENTER,
        "Demo preview - back: chevron or left-edge swipe",
        theme::font(10.5),
        theme::white(0.35),
        false,
    );
}

fn browser_mock(p: &Painter, c: Rect, accent: Color32) {
    let f = Rect::from_min_size(
        pos2(c.left() + 20.0, c.top() + 18.0),
        vec2(c.width() - 40.0, 40.0),
    );
    icons::rrect(p, f, 20.0, theme::white(0.10));
    p.circle_filled(pos2(f.left() + 22.0, f.center().y), 3.5, accent);
    label(
        p,
        pos2(f.left() + 36.0, f.center().y),
        Align2::LEFT_CENTER,
        "google.com/search",
        theme::font(13.5),
        theme::white(0.85),
        false,
    );
    for i in 0..3 {
        let y = c.top() + 78.0 + i as f32 * 74.0;
        icons::disc(p, pos2(c.left() + 40.0, y + 14.0), 14.0, theme::with_alpha(accent, 0.85));
        bar(p, c.left() + 64.0, y + 2.0, c.width() * 0.55, 9.0, theme::white(0.16));
        bar(p, c.left() + 64.0, y + 18.0, c.width() * 0.40, 7.0, theme::white(0.10));
        bar(p, c.left() + 64.0, y + 32.0, c.width() * 0.30, 7.0, theme::with_alpha(accent, 0.5));
    }
}

fn chat_mock(p: &Painter, c: Rect, accent: Color32, app: App) {
    label(
        p,
        pos2(c.left() + 24.0, c.top() + 16.0),
        Align2::LEFT_CENTER,
        if app == App::Forums { "OnePlus Forums" } else { "Aarav" },
        theme::font_medium(14.0),
        theme::white(0.9),
        false,
    );
    let bubbles: [(bool, f32, &str); 5] = [
        (false, 0.50, "Did you try the new build?"),
        (true, 0.46, "Running it now - feels stock"),
        (false, 0.56, "Cards update via postCard()"),
        (true, 0.40, "Hidden Space works too"),
        (false, 0.44, "Unhide me from there :)"),
    ];
    let mut y = c.top() + 42.0;
    for (out, wf, txt) in bubbles {
        let w = c.width() * wf;
        let r = Rect::from_min_size(
            pos2(
                if out { c.right() - 20.0 - w } else { c.left() + 20.0 },
                y,
            ),
            vec2(w, 44.0),
        );
        icons::rrect(
            p,
            r,
            16.0,
            if out {
                theme::with_alpha(accent, 0.85)
            } else {
                theme::white(0.12)
            },
        );
        label(p, r.center(), Align2::CENTER_CENTER, txt, theme::font(11.5), Color32::WHITE, false);
        y += 56.0;
    }
    let _ = y;
    let inp = Rect::from_min_size(
        pos2(c.left() + 20.0, c.bottom() - 30.0),
        vec2(c.width() - 40.0, 44.0),
    );
    icons::rrect(p, inp, 22.0, theme::white(0.10));
    label(
        p,
        pos2(inp.left() + 18.0, inp.center().y),
        Align2::LEFT_CENTER,
        "Message",
        theme::font(13.0),
        theme::white(0.5),
        false,
    );
    icons::glyph_play(
        p,
        pos2(inp.right() - 24.0, inp.center().y),
        14.0,
        theme::with_alpha(accent, 0.9),
    );
}

fn camera_mock(p: &Painter, c: Rect, hits: &mut Vec<(Rect, Hit)>) {
    icons::rrect(p, c, 0.0, theme::rgb(0x05070A));
    // Viewfinder corner brackets.
    let b = 34.0;
    for (px, py, sx, sy) in [
        (c.left() + 28.0, c.top() + 28.0, 1.0, 1.0),
        (c.right() - 28.0, c.top() + 28.0, -1.0, 1.0),
        (c.left() + 28.0, c.bottom() - 96.0, 1.0, -1.0),
        (c.right() - 28.0, c.bottom() - 96.0, -1.0, -1.0),
    ] {
        let o = pos2(px, py);
        icons::line(p, o, o + vec2(b * sx, 0.0), 2.5, theme::white(0.6));
        icons::line(p, o, o + vec2(0.0, b * sy), 2.5, theme::white(0.6));
    }
    // Focus square.
    let fs = Rect::from_center_size(
        pos2(c.center().x, c.center().y - 40.0),
        vec2(96.0, 96.0),
    );
    let fb = 16.0;
    for (px, py, sx, sy) in [
        (fs.left(), fs.top(), 1.0, 1.0),
        (fs.right(), fs.top(), -1.0, 1.0),
        (fs.left(), fs.bottom(), 1.0, -1.0),
        (fs.right(), fs.bottom(), -1.0, -1.0),
    ] {
        let o = pos2(px, py);
        icons::line(p, o, o + vec2(fb * sx, 0.0), 2.0, theme::rgb(0xEB0029));
        icons::line(p, o, o + vec2(0.0, fb * sy), 2.0, theme::rgb(0xEB0029));
    }
    // Mode strip.
    label(
        p,
        pos2(c.center().x, c.bottom() - 132.0),
        Align2::CENTER_CENTER,
        "VIDEO     PORTRAIT     PHOTO",
        theme::font_medium(11.0),
        theme::white(0.8),
        false,
    );
    // Shutter + flip.
    let sc = pos2(c.center().x, c.bottom() - 70.0);
    icons::glyph_shutter(p, sc, 30.0, Color32::WHITE);
    hits.push((Rect::from_center_size(sc, vec2(84.0, 84.0)), Hit::AppAction(0)));
    let fc = pos2(c.right() - 64.0, c.bottom() - 70.0);
    p.circle_filled(fc, 20.0, theme::white(0.12));
    icons::glyph_rotate(p, fc, 22.0, theme::white(0.85));
    hits.push((Rect::from_center_size(fc, vec2(64.0, 64.0)), Hit::AppAction(1)));
}

fn gallery_mock(p: &Painter, c: Rect) {
    label(
        p,
        pos2(c.left() + 24.0, c.top() + 18.0),
        Align2::LEFT_CENTER,
        "Today",
        theme::font_medium(15.0),
        theme::white(0.9),
        false,
    );
    const HUE: [u32; 12] = [
        0x2E5C8A, 0x8A4B2E, 0x3E7A52, 0x8A2E4C, 0x5C6672, 0x2E8A82, 0x7A6A2E, 0x4C2E8A,
        0x8A7A2E, 0x2E4C8A, 0x6A2E8A, 0x2E8A4C,
    ];
    let top = c.top() + 40.0;
    let gap = 6.0;
    let tw = (c.width() - 40.0 - gap * 2.0) / 3.0;
    let th = tw * 1.12;
    for i in 0..12 {
        let col = i % 3;
        let row = i / 3;
        let tr = Rect::from_min_size(
            pos2(
                c.left() + 20.0 + col as f32 * (tw + gap),
                top + row as f32 * (th + gap),
            ),
            vec2(tw, th),
        );
        if tr.bottom() > c.bottom() - 10.0 {
            break;
        }
        icons::rrect(p, tr, 8.0, theme::rgb(HUE[i]));
        p.circle_filled(
            pos2(tr.right() - 14.0, tr.top() + 14.0),
            6.0,
            theme::white(0.25),
        );
    }
}

fn store_mock(p: &Painter, c: Rect, hits: &mut Vec<(Rect, Hit)>) {
    label(
        p,
        pos2(c.left() + 24.0, c.top() + 18.0),
        Align2::LEFT_CENTER,
        "Recommended for you",
        theme::font_medium(15.0),
        theme::white(0.9),
        false,
    );
    const NAMES: [&str; 3] = ["Launcher Lab", "Retro Notes", "Cloud Sync"];
    for i in 0..3 {
        let y = c.top() + 48.0 + i as f32 * 84.0;
        icons::disc(p, pos2(c.left() + 44.0, y + 26.0), 17.0, theme::white(0.14));
        label(
            p,
            pos2(c.left() + 44.0, y + 26.0),
            Align2::CENTER_CENTER,
            &format!("{}", i + 1),
            theme::font_medium(13.0),
            theme::white(0.8),
            false,
        );
        label(
            p,
            pos2(c.left() + 74.0, y + 16.0),
            Align2::LEFT_CENTER,
            NAMES[i],
            theme::font_medium(14.0),
            theme::white(0.95),
            false,
        );
        bar(p, c.left() + 74.0, y + 34.0, c.width() * 0.38, 7.0, theme::white(0.10));
        let pill = Rect::from_center_size(pos2(c.right() - 66.0, y + 26.0), vec2(84.0, 34.0));
        icons::rrect(p, pill, 17.0, theme::rgb(0x1F9C6C));
        label(
            p,
            pill.center(),
            Align2::CENTER_CENTER,
            "Install",
            theme::font_medium(12.5),
            Color32::WHITE,
            false,
        );
        hits.push((pill, Hit::AppAction(1 + i)));
    }
}

fn music_mock(p: &Painter, c: Rect, accent: Color32, hits: &mut Vec<(Rect, Hit)>) {
    let size = (c.width() - 96.0).clamp(140.0, 220.0);
    let art = Rect::from_center_size(
        pos2(c.center().x, c.top() + 38.0 + size * 0.5),
        vec2(size, size),
    );
    icons::rrect(p, art, 18.0, theme::with_alpha(accent, 0.92));
    icons::rrect(p, art.shrink(size * 0.22), 12.0, theme::black(0.25));
    p.circle_filled(art.center(), size * 0.10, theme::white(0.9));
    p.circle_filled(art.center(), size * 0.045, theme::rgb(0x111111));
    label(
        p,
        pos2(c.center().x, art.bottom() + 28.0),
        Align2::CENTER_CENTER,
        "Reconstruction EP",
        theme::font_medium(16.0),
        Color32::WHITE,
        false,
    );
    label(
        p,
        pos2(c.center().x, art.bottom() + 50.0),
        Align2::CENTER_CENTER,
        "rajbhx",
        theme::font(12.5),
        theme::white(0.6),
        false,
    );
    let seek = Rect::from_min_size(
        pos2(c.left() + 36.0, art.bottom() + 74.0),
        vec2(c.width() - 72.0, 5.0),
    );
    icons::rrect(p, seek, 2.5, theme::white(0.15));
    icons::rrect(
        p,
        Rect::from_min_max(seek.min, pos2(seek.left() + seek.width() * 0.35, seek.max.y)),
        2.5,
        theme::with_alpha(accent, 0.95),
    );
    p.circle_filled(
        pos2(seek.left() + seek.width() * 0.35, seek.center().y),
        7.0,
        Color32::WHITE,
    );
    let cc = pos2(c.center().x, seek.bottom() + 52.0);
    icons::glyph_skip(p, pos2(cc.x - 86.0, cc.y), 24.0, theme::white(0.85), true);
    p.circle_filled(cc, 30.0, theme::with_alpha(accent, 0.95));
    icons::glyph_pause(p, cc, 22.0, Color32::WHITE);
    hits.push((Rect::from_center_size(cc, vec2(80.0, 80.0)), Hit::AppAction(1)));
    icons::glyph_skip(p, pos2(cc.x + 86.0, cc.y), 24.0, theme::white(0.85), false);
}

fn weather_mock(p: &Painter, c: Rect, accent: Color32, hits: &mut Vec<(Rect, Hit)>) {
    label(
        p,
        pos2(c.left() + 28.0, c.top() + 40.0),
        Align2::LEFT_CENTER,
        "84\u{00B0}",
        theme::font_light(64.0),
        Color32::WHITE,
        false,
    );
    label(
        p,
        pos2(c.left() + 30.0, c.top() + 92.0),
        Align2::LEFT_CENTER,
        "CLOUDY",
        theme::font_medium(13.0),
        theme::with_alpha(accent, 0.95),
        false,
    );
    label(
        p,
        pos2(c.left() + 30.0, c.top() + 114.0),
        Align2::LEFT_CENTER,
        "Cloudy with sunny intervals",
        theme::font(12.5),
        theme::white(0.6),
        false,
    );
    let days = ["MON", "TUE", "WED", "THU", "FRI"];
    let temps = ["84/71", "83/70", "85/72", "82/69", "81/68"];
    let col_w = (c.width() - 48.0) / 5.0;
    for i in 0..5 {
        let cx = c.left() + 24.0 + col_w * (i as f32 + 0.5);
        let y = c.top() + 170.0;
        label(
            p,
            pos2(cx, y),
            Align2::CENTER_CENTER,
            days[i],
            theme::font_medium(11.0),
            theme::white(0.55),
            false,
        );
        label(
            p,
            pos2(cx, y + 24.0),
            Align2::CENTER_CENTER,
            temps[i],
            theme::font(13.0),
            theme::white(0.9),
            false,
        );
    }
    let card = Rect::from_min_size(
        pos2(c.left() + 24.0, c.top() + 226.0),
        vec2(c.width() - 48.0, 124.0),
    );
    icons::rrect(p, card, 18.0, theme::white(0.06));
    label(
        p,
        pos2(card.left() + 18.0, card.top() + 22.0),
        Align2::LEFT_CENTER,
        "Feels like 86\u{00B0}",
        theme::font_medium(14.0),
        theme::white(0.9),
        false,
    );
    label(
        p,
        pos2(card.left() + 18.0, card.top() + 46.0),
        Align2::LEFT_CENTER,
        "Humidity 62%  -  Wind 9 km/h",
        theme::font(12.5),
        theme::white(0.6),
        false,
    );
    label(
        p,
        pos2(card.left() + 18.0, card.top() + 70.0),
        Align2::LEFT_CENTER,
        "Sunrise 06:12  -  Sunset 18:44",
        theme::font(12.5),
        theme::white(0.6),
        false,
    );
    label(
        p,
        pos2(card.left() + 18.0, card.top() + 96.0),
        Align2::LEFT_CENTER,
        "Tap to refresh (getCard weather.today)",
        theme::font(11.5),
        theme::with_alpha(accent, 0.9),
        false,
    );
    hits.push((card, Hit::AppAction(2)));
}

fn clock_mock(p: &Painter, c: Rect, app: &LauncherLab) {
    label(
        p,
        pos2(c.center().x, c.top() + 110.0),
        Align2::CENTER_CENTER,
        &clock::clock_label(app.time),
        theme::font_light(72.0),
        Color32::WHITE,
        false,
    );
    label(
        p,
        pos2(c.center().x, c.top() + 166.0),
        Align2::CENTER_CENTER,
        &clock::date_label(app.time),
        theme::font(14.0),
        theme::white(0.65),
        false,
    );
    let rows = [
        ("London", "08:04", "+5h"),
        ("New York", "03:04", "-4h"),
        ("Tokyo", "16:04", "+13h"),
    ];
    let mut y = c.top() + 236.0;
    for (city, t, off) in rows {
        label(
            p,
            pos2(c.left() + 28.0, y),
            Align2::LEFT_CENTER,
            city,
            theme::font_medium(14.0),
            theme::white(0.9),
            false,
        );
        label(
            p,
            pos2(c.right() - 76.0, y),
            Align2::RIGHT_CENTER,
            off,
            theme::font(12.0),
            theme::white(0.45),
            false,
        );
        label(
            p,
            pos2(c.right() - 28.0, y),
            Align2::RIGHT_CENTER,
            t,
            theme::font(20.0),
            theme::white(0.9),
            false,
        );
        bar(p, c.left() + 28.0, y + 22.0, c.width() - 56.0, 1.0, theme::white(0.08));
        y += 64.0;
    }
}

fn calc_mock(p: &Painter, c: Rect, hits: &mut Vec<(Rect, Hit)>) {
    label(
        p,
        pos2(c.right() - 28.0, c.top() + 80.0),
        Align2::RIGHT_CENTER,
        "0",
        theme::font_light(56.0),
        Color32::WHITE,
        false,
    );
    label(
        p,
        pos2(c.right() - 30.0, c.top() + 122.0),
        Align2::RIGHT_CENTER,
        "demo display",
        theme::font(11.0),
        theme::white(0.4),
        false,
    );
    let keys: [[&str; 4]; 5] = [
        ["AC", "()", "%", "\u{00F7}"],
        ["7", "8", "9", "\u{00D7}"],
        ["4", "5", "6", "-"],
        ["1", "2", "3", "+"],
        ["0", ".", "=", ""],
    ];
    let gap = 10.0;
    let kw = (c.width() - 48.0 - gap * 3.0) / 4.0;
    let kh = (kw * 0.82).min(64.0);
    let x0 = c.left() + 24.0;
    let grid_top = c.bottom() - 5.0 * (kh + gap) - 8.0;
    for r in 0..5 {
        for col in 0..4 {
            let key = keys[r][col];
            if key.is_empty() {
                continue;
            }
            let kr = Rect::from_min_size(
                pos2(x0 + col as f32 * (kw + gap), grid_top + r as f32 * (kh + gap)),
                vec2(kw, kh),
            );
            let op = r == 0 || col == 3 || (r == 4 && col == 2);
            icons::rrect(
                p,
                kr,
                kh * 0.5,
                if op {
                    theme::with_alpha(theme::rgb(0xFF7A1A), 0.20)
                } else {
                    theme::white(0.09)
                },
            );
            label(
                p,
                kr.center(),
                Align2::CENTER_CENTER,
                key,
                theme::font_medium(19.0),
                if op {
                    theme::rgb(0xFF9A4A)
                } else {
                    Color32::WHITE
                },
                false,
            );
            let idx = r * 4 + col;
            hits.push((kr, Hit::AppAction(10 + idx)));
        }
    }
}

fn maps_mock(p: &Painter, c: Rect, accent: Color32) {
    for i in 0..7 {
        let y = c.top() + 40.0 + i as f32 * 74.0;
        bar(p, c.left(), y, c.width(), 8.0, theme::white(0.05));
    }
    for i in 0..5 {
        let x = c.left() + 30.0 + i as f32 * 92.0;
        icons::rrect(
            p,
            Rect::from_min_size(pos2(x, c.top() + 30.0), vec2(8.0, c.height() - 60.0)),
            4.0,
            theme::white(0.05),
        );
    }
    let route = [
        pos2(c.left() + 60.0, c.bottom() - 120.0),
        pos2(c.left() + 60.0, c.top() + 260.0),
        pos2(c.center().x + 40.0, c.top() + 260.0),
        pos2(c.center().x + 40.0, c.top() + 150.0),
    ];
    icons::pline(p, &route, 6.0, theme::with_alpha(accent, 0.85));
    icons::glyph_pin(
        p,
        pos2(c.center().x + 40.0, c.top() + 138.0),
        22.0,
        theme::rgb(0xEA4335),
    );
    let f = Rect::from_min_size(
        pos2(c.left() + 20.0, c.top() + 16.0),
        vec2(c.width() - 40.0, 42.0),
    );
    icons::rrect(p, f, 21.0, theme::white(0.10));
    icons::glyph_magnifier(p, pos2(f.left() + 24.0, f.center().y), 18.0, theme::white(0.6));
    label(
        p,
        pos2(f.left() + 44.0, f.center().y),
        Align2::LEFT_CENTER,
        "Coffee nearby",
        theme::font(13.5),
        theme::white(0.8),
        false,
    );
}

fn video_mock(p: &Painter, c: Rect, accent: Color32) {
    const TITLES: [&str; 2] = ["OnePlus Launcher teardown", "I built a launcher in Rust"];
    for i in 0..2 {
        let y = c.top() + 26.0 + i as f32 * 230.0;
        let thumb = Rect::from_min_size(
            pos2(c.left() + 16.0, y),
            vec2(c.width() - 32.0, (c.width() - 32.0) * 0.5625),
        );
        icons::rrect(p, thumb, 12.0, theme::with_alpha(accent, 0.35));
        p.circle_filled(thumb.center(), 26.0, theme::black(0.45));
        icons::glyph_play(p, thumb.center() + vec2(2.0, 0.0), 22.0, Color32::WHITE);
        label(
            p,
            pos2(c.left() + 18.0, thumb.bottom() + 20.0),
            Align2::LEFT_CENTER,
            TITLES[i],
            theme::font_medium(14.0),
            theme::white(0.9),
            false,
        );
        bar(p, c.left() + 18.0, thumb.bottom() + 34.0, c.width() * 0.45, 7.0, theme::white(0.10));
    }
}

fn skeleton_mock(p: &Painter, c: Rect, accent: Color32) {
    for i in 0..5 {
        let y = c.top() + 30.0 + i as f32 * 72.0;
        icons::disc(p, pos2(c.left() + 46.0, y + 22.0), 18.0, theme::with_alpha(accent, 0.85));
        bar(p, c.left() + 78.0, y + 10.0, c.width() * 0.48, 10.0, theme::white(0.14));
        bar(p, c.left() + 78.0, y + 28.0, c.width() * 0.34, 8.0, theme::white(0.08));
        icons::glyph_chev(p, pos2(c.right() - 30.0, y + 22.0), 18.0, theme::white(0.35));
        bar(p, c.left() + 20.0, y + 58.0, c.width() - 40.0, 1.0, theme::white(0.06));
    }
}

// -------------------------------------------------------------- onboarding

fn draw_onboarding(p: &Painter, rect: Rect, hits: &mut Vec<(Rect, Hit)>, pal: &Palette) {
    p.rect_filled(rect, CornerRadius::ZERO, theme::black(0.78));
    let card_h = (rect.height() - 160.0).min(560.0);
    let card = Rect::from_center_size(
        pos2(rect.center().x, rect.top() + 60.0 + card_h * 0.5),
        vec2(rect.width() - 44.0, card_h),
    );
    icons::rrect(p, card.translate(vec2(0.0, 6.0)), 24.0, theme::black(0.5));
    icons::rrect(p, card, 24.0, pal.panel);
    label(
        p,
        pos2(card.center().x, card.top() + 34.0),
        Align2::CENTER_CENTER,
        "LAUNCHER LAB",
        theme::font_medium(16.0),
        theme::with_alpha(pal.accent, 0.95),
        false,
    );
    label(
        p,
        pos2(card.center().x, card.top() + 58.0),
        Align2::CENTER_CENTER,
        "OnePlus Launcher 4.4.8 as a pure-Rust reconstruction",
        theme::font(11.5),
        theme::with_alpha(pal.text, 0.6),
        false,
    );
    const ROWS: [&str; 8] = [
        "Swipe up: app drawer with all apps",
        "Swipe up from the bottom edge: recents",
        "Swipe down from the status bar: notification shade",
        "Swipe right on page 1 or the left edge: Shelf",
        "In the drawer, swipe right: Hidden Space",
        "Hold an app in the drawer: hide it",
        "Hold empty home: light / dark chrome",
        "Double-tap empty home: lock screen",
    ];
    let mut y = card.top() + 92.0;
    for row in ROWS {
        p.circle_filled(
            pos2(card.left() + 30.0, y),
            3.2,
            theme::with_alpha(pal.accent, 0.9),
        );
        label(
            p,
            pos2(card.left() + 46.0, y),
            Align2::LEFT_CENTER,
            row,
            theme::font(12.5),
            theme::with_alpha(pal.text, 0.88),
            false,
        );
        y += 34.0;
    }
    let btn = Rect::from_center_size(pos2(card.center().x, y + 30.0), vec2(180.0, 46.0));
    icons::rrect(p, btn, 23.0, theme::with_alpha(pal.accent, 0.95));
    label(
        p,
        btn.center(),
        Align2::CENTER_CENTER,
        "Got it",
        theme::font_medium(15.0),
        Color32::WHITE,
        false,
    );
    hits.push((card, Hit::OnboardOk));
    hits.push((btn, Hit::OnboardOk));
}

//! All rendering. Painter-only: every control is a hand-drawn primitive plus
//! a hit rectangle that input routing in `crate::app` can pick up.

use eframe::egui::{
    pos2, vec2, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke,
};

use crate::app::LauncherLab;
use crate::model::state::Screen;
use crate::theme;

/// Interactive targets, rebuilt every frame during painting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hit {
    Tile(usize),
    PageDot(usize),
    LockChip,
    UnlockChip,
    ShelfOpenChip,
    CloseShelf,
    MemoAdd,
    ShelfCard(String),
    Task(usize),
    HomeChip,
    HiddenClose,
    PasswordToggle,
    UnlockHold,
    AssistantOk,
}

pub fn paint(app: &mut LauncherLab, p: &Painter, rect: Rect) {
    app.hits.clear();

    if app.overview_t > 0.01 {
        draw_overview(app, p, rect);
    }
    draw_home(app, p, rect);
    if app.shelf_t > 0.01 {
        draw_shelf(app, p, rect);
    }
    if app.hidden_t > 0.01 {
        draw_hidden(app, p, rect);
    }
    if app.assistant_ttl > 0.0 {
        draw_assistant(app, p, rect);
    }
    if app.lock_overlay_t > 0.01 {
        draw_locked(app, p, rect);
    }

    draw_status(app, p, rect);
    draw_hud(app, p, rect);

    if let Some((msg, ttl)) = &app.toast {
        draw_toast(p, rect, msg, *ttl);
    }
}

// ---------------------------------------------------------------- helpers

fn est_w(s: &str, size: f32) -> f32 {
    s.chars().count() as f32 * size * 0.54
}

fn glyph_tile(p: &Painter, r: Rect, accent: u32, label: &str) {
    let radius = (r.width() * 0.24) as u8;
    p.rect_filled(r, CornerRadius::same(radius), theme::rgb(accent));
    let initial: String = label.chars().take(1).collect();
    p.text(
        r.center(),
        Align2::CENTER_CENTER,
        initial,
        FontId::proportional(r.width() * 0.42),
        Color32::WHITE,
    );
}

fn hhmm() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mins = (secs / 60) % (24 * 60);
    format!("{:02}:{:02}", mins / 60, mins % 60)
}

// ------------------------------------------------------------------ home

fn draw_home(app: &mut LauncherLab, p: &Painter, rect: Rect) {
    if app.overview_t > 0.99 {
        return;
    }
    let r = rect.translate(vec2(0.0, -rect.height() * 0.06 * app.overview_t));
    let w = r.width();

    let margin = w * 0.06;
    let gap = w * 0.030;
    let cell_w = (w - 2.0 * margin - 3.0 * gap) / 4.0;
    let label_h = w * 0.072;
    let row_h = cell_w + gap + label_h;
    let top = r.top() + r.height() * 0.12;

    let cam = app.pager.camera_x(w);
    let first = app.pager.current.saturating_sub(1);
    let last = (app.pager.current + 1).min(app.pager.pages - 1);

    for pg in first..=last {
        let page_r = r.translate(vec2(pg as f32 * w - cam, 0.0));
        draw_home_page(app, p, page_r, pg, margin, gap, cell_w, label_h, row_h, top);
    }

    // page dots
    let dots_y = r.bottom() - r.height() * 0.085;
    let pages = app.pager.pages;
    let total = pages as f32 * w * 0.05;
    for i in 0..pages {
        let cx = r.center().x - total / 2.0 + (i as f32 + 0.5) * w * 0.05;
        let active = i == app.pager.current;
        let dot_r = if active { w * 0.006 } else { w * 0.005 };
        let color = if active { theme::accent() } else { theme::subtext() };
        p.circle_filled(pos2(cx, dots_y), dot_r, color);
        app.hits.push((
            Rect::from_center_size(pos2(cx, dots_y), vec2(w * 0.04, w * 0.04)),
            Hit::PageDot(i),
        ));
    }

    // hint
    p.text(
        pos2(r.center().x, r.bottom() - r.height() * 0.038),
        Align2::CENTER_CENTER,
        "swipe up: recents   |   left edge: shelf   |   pinch out: hidden",
        FontId::proportional(w * 0.033),
        theme::subtext(),
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_home_page(
    app: &mut LauncherLab,
    p: &Painter,
    page_r: Rect,
    pg: usize,
    margin: f32,
    gap: f32,
    cell_w: f32,
    label_h: f32,
    row_h: f32,
    top: f32,
) {
    for i in 0..16 {
        let idx = pg * 16 + i;
        let Some(app_data) = app.apps.get(idx).cloned() else {
            break;
        };
        let col = (i % 4) as f32;
        let row = (i / 4) as f32;
        let x = page_r.left() + margin + col * (cell_w + gap);
        let y = top + row * (row_h + gap);
        let icon = Rect::from_min_size(pos2(x, y), vec2(cell_w, cell_w));

        if !icon.intersects(page_r) {
            continue;
        }

        glyph_tile(p, icon, app_data.accent, &app_data.name);
        p.text(
            pos2(x + cell_w / 2.0, y + cell_w + gap * 0.45),
            Align2::CENTER_TOP,
            &app_data.name,
            FontId::proportional(label_h * 0.62),
            theme::text(),
        );

        let hit_r = Rect::from_min_size(pos2(x, y), vec2(cell_w, cell_w + label_h));
        app.hits.push((hit_r, Hit::Tile(idx)));
    }
}

// -------------------------------------------------------------- overview

fn draw_overview(app: &mut LauncherLab, p: &Painter, rect: Rect) {
    let t = app.overview_t;
    let panel = rect.translate(vec2(0.0, (1.0 - t) * rect.height()));
    let w = panel.width();
    let h = panel.height();

    p.rect_filled(
        rect,
        CornerRadius::same(0),
        theme::with_alpha(Color32::BLACK, 0.45 * t),
    );
    p.rect_filled(panel, CornerRadius::same(24), theme::surface());

    p.text(
        pos2(panel.center().x, panel.top() + h * 0.055),
        Align2::CENTER_CENTER,
        "RECENTS",
        FontId::proportional(w * 0.05),
        theme::text(),
    );
    p.text(
        pos2(panel.center().x, panel.top() + h * 0.085),
        Align2::CENTER_CENTER,
        "swipe down to reset - drag a card sideways to dismiss",
        FontId::proportional(w * 0.030),
        theme::subtext(),
    );

    let card_w = w * 0.84;
    let card_h = h * 0.145;
    let drag_idx = app.task_drag.map(|(i, _)| i);
    let drag_x = app.task_drag.map(|(_, x)| x).unwrap_or(0.0);

    for (i, task) in app.tasks.iter().enumerate() {
        let x_off = if drag_idx == Some(i) { drag_x } else { task.x_off };
        let scale = (1.0 - task.dismiss_t) * (1.0 + 0.035 * task.resume_t);
        let center = pos2(
            panel.center().x + x_off,
            panel.top() + h * 0.135 + i as f32 * (card_h + h * 0.022) + card_h / 2.0,
        );
        let card = Rect::from_center_size(center, vec2(card_w, card_h) * scale);

        p.rect_filled(card, CornerRadius::same(22), theme::surface_hi());
        let bar = Rect::from_min_size(card.min, vec2(6.0, card.height()));
        p.rect_filled(bar, CornerRadius::same(3), theme::rgb(task.accent));

        p.text(
            pos2(card.left() + w * 0.05, card.center().y - card_h * 0.10),
            Align2::LEFT_CENTER,
            &task.title,
            FontId::proportional(card_h * 0.30),
            theme::text(),
        );
        p.text(
            pos2(card.left() + w * 0.05, card.center().y + card_h * 0.16),
            Align2::LEFT_CENTER,
            &task.caption,
            FontId::proportional(card_h * 0.21),
            theme::subtext(),
        );

        let ic = pos2(card.right() - w * 0.09, card.center().y);
        let ir = w * 0.052;
        p.circle_filled(ic, ir, theme::rgb(task.accent));
        let initial: String = task.title.chars().take(1).collect();
        p.text(
            ic,
            Align2::CENTER_CENTER,
            initial,
            FontId::proportional(ir * 0.9),
            Color32::WHITE,
        );

        if scale > 0.2 {
            app.hits.push((card, Hit::Task(i)));
        }
    }

    // home chip
    let chip = Rect::from_center_size(
        pos2(panel.center().x, panel.bottom() - h * 0.055),
        vec2(w * 0.30, h * 0.045),
    );
    p.rect_filled(chip, CornerRadius::same((chip.height() * 0.5) as u8), theme::surface_hi());
    p.circle_filled(
        pos2(chip.left() + chip.height() * 0.6, chip.center().y),
        chip.height() * 0.22,
        theme::accent(),
    );
    p.text(
        pos2(chip.center().x + chip.height() * 0.1, chip.center().y),
        Align2::CENTER_CENTER,
        "Home",
        FontId::proportional(chip.height() * 0.5),
        theme::text(),
    );
    app.hits.push((chip, Hit::HomeChip));
}

// ----------------------------------------------------------------- shelf

fn draw_shelf(app: &mut LauncherLab, p: &Painter, rect: Rect) {
    let t = app.shelf_t;
    let w = rect.width();
    let h = rect.height();

    p.rect_filled(
        rect,
        CornerRadius::same(0),
        theme::with_alpha(Color32::BLACK, 0.42 * t),
    );

    let panel_w = w * 0.92;
    let panel = Rect::from_min_size(
        pos2(rect.left() - panel_w * (1.0 - t), rect.top()),
        vec2(panel_w, h),
    );
    let cp = p.with_clip_rect(panel);
    cp.rect_filled(panel, CornerRadius::same(0), theme::surface());

    cp.text(
        pos2(panel.left() + panel_w * 0.06, panel.top() + h * 0.05),
        Align2::LEFT_TOP,
        "Shelf",
        FontId::proportional(w * 0.065),
        theme::text(),
    );
    cp.text(
        pos2(panel.left() + panel_w * 0.06, panel.top() + h * 0.092),
        Align2::LEFT_TOP,
        "channel cards (CardContract)",
        FontId::proportional(w * 0.032),
        theme::subtext(),
    );

    // close chip
    let close = Rect::from_center_size(
        pos2(panel.right() - panel_w * 0.08, panel.top() + h * 0.062),
        vec2(w * 0.09, w * 0.09),
    );
    cp.rect_filled(close, CornerRadius::same((w * 0.045) as u8), theme::surface_hi());
    cp.text(
        close.center(),
        Align2::CENTER_CENTER,
        "x",
        FontId::proportional(w * 0.05),
        theme::text(),
    );
    app.hits.push((close, Hit::CloseShelf));

    // cards
    let mut y = panel.top() + h * 0.13;
    let card_h = h * 0.115;
    let margin = panel_w * 0.06;
    let cards: Vec<(String, String, String, String, u32)> = app
        .cards
        .cards()
        .iter()
        .map(|c| {
            (
                c.channel.clone(),
                c.tag.clone(),
                c.title.clone(),
                c.body.clone(),
                c.accent,
            )
        })
        .collect();

    for (channel, tag, title, body, accent) in cards {
        let card = Rect::from_min_size(pos2(panel.left() + margin, y), vec2(panel_w - margin * 2.0, card_h));
        if card.top() > panel.bottom() - h * 0.16 {
            break;
        }
        cp.rect_filled(card, CornerRadius::same(20), theme::surface_hi());
        let bar = Rect::from_min_size(card.min, vec2(5.0, card.height()));
        cp.rect_filled(bar, CornerRadius::same(2), theme::rgb(accent));

        cp.text(
            pos2(card.left() + margin * 0.5, card.center().y - card_h * 0.16),
            Align2::LEFT_CENTER,
            &title,
            FontId::proportional(card_h * 0.30),
            theme::text(),
        );
        cp.text(
            pos2(card.left() + margin * 0.5, card.center().y + card_h * 0.20),
            Align2::LEFT_CENTER,
            &body,
            FontId::proportional(card_h * 0.22),
            theme::subtext(),
        );
        cp.text(
            pos2(card.right() - margin * 0.3, card.center().y),
            Align2::RIGHT_CENTER,
            format!("{channel}.{tag}"),
            FontId::proportional(card_h * 0.17),
            theme::subtext(),
        );

        if channel == "memo" {
            app.hits.push((card, Hit::ShelfCard(tag)));
        }
        y += card_h + h * 0.018;
    }

    // add-memo button
    let add = Rect::from_center_size(
        pos2(panel.center().x, panel.bottom() - h * 0.10),
        vec2(panel_w * 0.62, h * 0.062),
    );
    p.rect_filled(add, CornerRadius::same((add.height() * 0.5) as u8), theme::accent());
    p.text(
        add.center(),
        Align2::CENTER_CENTER,
        "+ MEMO",
        FontId::proportional(add.height() * 0.42),
        Color32::WHITE,
    );
    app.hits.push((add, Hit::MemoAdd));

    p.text(
        pos2(panel.center().x, panel.bottom() - h * 0.035),
        Align2::CENTER_CENTER,
        "tap memo: getCard   -   hold memo: deleteCard",
        FontId::proportional(w * 0.030),
        theme::subtext(),
    );
}

// ---------------------------------------------------------------- hidden

fn draw_hidden(app: &mut LauncherLab, p: &Painter, rect: Rect) {
    let t = app.hidden_t;
    let panel = Rect::from_center_size(rect.center(), rect.size() * (0.3 + 0.7 * t));
    let cp = p.with_clip_rect(panel);
    let w = rect.width();
    let h = rect.height();

    cp.rect_filled(rect, CornerRadius::same(0), theme::rgb(0x07070A));

    cp.text(
        pos2(rect.center().x, rect.top() + h * 0.10),
        Align2::CENTER_CENTER,
        "Hidden Space",
        FontId::proportional(w * 0.06),
        theme::text(),
    );
    cp.text(
        pos2(rect.center().x, rect.top() + h * 0.135),
        Align2::CENTER_CENTER,
        if app.hidden.is_unlocked() {
            "pinch in to exit"
        } else {
            "password protected"
        },
        FontId::proportional(w * 0.032),
        theme::subtext(),
    );

    // close chip
    let close = Rect::from_center_size(
        pos2(rect.right() - w * 0.10, rect.top() + h * 0.085),
        vec2(w * 0.10, w * 0.10),
    );
    cp.rect_filled(close, CornerRadius::same((w * 0.05) as u8), theme::surface_hi());
    cp.text(
        close.center(),
        Align2::CENTER_CENTER,
        "x",
        FontId::proportional(w * 0.055),
        theme::text(),
    );
    app.hits.push((close, Hit::HiddenClose));

    if app.hidden.is_unlocked() {
        // hidden item tiles
        let items: Vec<(String, u32)> = app
            .hidden
            .items()
            .iter()
            .map(|i| (i.name.clone(), i.accent))
            .collect();
        let tile = w * 0.30;
        let gx = w * 0.06;
        let gy = w * 0.06;
        let total_w = tile * 2.0 + gx;
        let start_x = rect.center().x - total_w / 2.0;
        let start_y = rect.top() + h * 0.22;
        for (i, (name, accent)) in items.iter().enumerate() {
            let col = (i % 2) as f32;
            let row = (i / 2) as f32;
            let tr = Rect::from_min_size(
                pos2(start_x + col * (tile + gx), start_y + row * (tile * 0.7 + gy)),
                vec2(tile, tile * 0.7),
            );
            cp.rect_filled(tr, CornerRadius::same(20), theme::surface());
            let ic = pos2(tr.left() + tile * 0.16, tr.center().y);
            cp.circle_filled(ic, w * 0.045, theme::rgb(*accent));
            cp.text(
                pos2(tr.left() + tile * 0.30, tr.center().y),
                Align2::LEFT_CENTER,
                name,
                FontId::proportional(tile * 0.19),
                theme::text(),
            );
        }
        cp.text(
            pos2(rect.center().x, rect.top() + h * 0.62),
            Align2::CENTER_CENTER,
            "not indexed by the home grid",
            FontId::proportional(w * 0.030),
            theme::subtext(),
        );
    } else {
        // hold-to-unlock pad
        let center = pos2(rect.center().x, rect.center().y);
        let radius = w * 0.15;
        let frac = (app.unlock_hold / 0.65).clamp(0.0, 1.0);
        ring(p, center, radius + w * 0.02, 1.0, theme::surface_hi(), 6.0);
        if frac > 0.0 {
            ring(p, center, radius + w * 0.02, frac, theme::accent(), 6.0);
        }
        cp.circle_filled(center, radius, theme::surface());
        // padlock glyph
        cp.rect_filled(
            Rect::from_center_size(center + vec2(0.0, w * 0.03), vec2(w * 0.10, w * 0.08)),
            CornerRadius::same(10),
            theme::accent(),
        );
        let shackle = Rect::from_center_size(center - vec2(0.0, w * 0.035), vec2(w * 0.06, w * 0.06));
        ring(p, shackle.center(), w * 0.03, 0.5, theme::subtext(), 5.0);
        p.text(
            pos2(center.x, center.y + radius + w * 0.09),
            Align2::CENTER_CENTER,
            "hold to unlock",
            FontId::proportional(w * 0.036),
            theme::text(),
        );
        let hold_r = Rect::from_center_size(center, vec2(radius * 2.4, radius * 2.4));
        app.hits.push((hold_r, Hit::UnlockHold));
    }

    // password toggle
    let toggle = Rect::from_center_size(
        pos2(rect.center().x, rect.bottom() - h * 0.10),
        vec2(w * 0.62, h * 0.055),
    );
    cp.rect_filled(toggle, CornerRadius::same((toggle.height() * 0.5) as u8), theme::surface_hi());
    let state = if app.hidden.password_enabled() { "ON" } else { "OFF" };
    let label = format!("Password: {state} - tap to toggle");
    p.text(
        toggle.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(toggle.height() * 0.42),
        theme::text(),
    );
    app.hits.push((toggle, Hit::PasswordToggle));
}

fn ring(p: &Painter, c: Pos2, r: f32, frac: f32, color: Color32, width: f32) {
    let n: i32 = 48;
    let start = -std::f32::consts::FRAC_PI_2;
    let seg = (frac.clamp(0.0, 1.0) * n as f32) as i32;
    for i in 0..seg {
        let a0 = start + (i as f32 / n as f32) * std::f32::consts::TAU;
        let a1 = start + ((i + 1) as f32 / n as f32) * std::f32::consts::TAU;
        p.line_segment(
            [
                c + vec2(a0.cos() * r, a0.sin() * r),
                c + vec2(a1.cos() * r, a1.sin() * r),
            ],
            Stroke::new(width, color),
        );
    }
}

// ------------------------------------------------------- chrome + overlays

fn draw_status(app: &mut LauncherLab, p: &Painter, rect: Rect) {
    if app.hidden_t > 0.05 {
        return;
    }
    let w = rect.width();
    let h = rect.height();
    let y = rect.top() + h * 0.028;

    p.text(
        pos2(rect.left() + w * 0.05, y),
        Align2::LEFT_CENTER,
        hhmm(),
        FontId::proportional(w * 0.042),
        theme::text(),
    );

    // battery
    let bat = Rect::from_min_size(
        pos2(rect.right() - w * 0.11, y - w * 0.016),
        vec2(w * 0.055, w * 0.032),
    );
    p.rect_filled(bat, CornerRadius::same(3), theme::subtext());
    let lvl = Rect::from_min_size(
        bat.min + vec2(2.0, 2.0),
        vec2((bat.width() - 4.0) * 0.86, bat.height() - 4.0),
    );
    p.rect_filled(lvl, CornerRadius::same(2), theme::ok());
    p.rect_filled(
        Rect::from_min_size(pos2(bat.right() + 1.0, bat.center().y - w * 0.007), vec2(2.0, w * 0.014)),
        CornerRadius::same(1),
        theme::subtext(),
    );

    // chips: SHELF + LOCK
    if app.screen != Screen::Hidden {
        let chip_h = w * 0.075;
        let lock_chip = Rect::from_center_size(
            pos2(rect.right() - w * 0.20, y),
            vec2(w * 0.14, chip_h),
        );
        p.rect_filled(lock_chip, CornerRadius::same((chip_h * 0.5) as u8), theme::surface_hi());
        p.text(
            lock_chip.center(),
            Align2::CENTER_CENTER,
            "LOCK",
            FontId::proportional(chip_h * 0.42),
            theme::subtext(),
        );
        app.hits.push((lock_chip, Hit::LockChip));

        let shelf_chip = Rect::from_center_size(
            pos2(rect.right() - w * 0.36, y),
            vec2(w * 0.14, chip_h),
        );
        p.rect_filled(shelf_chip, CornerRadius::same((chip_h * 0.5) as u8), theme::surface_hi());
        p.text(
            shelf_chip.center(),
            Align2::CENTER_CENTER,
            "SHELF",
            FontId::proportional(chip_h * 0.42),
            theme::subtext(),
        );
        app.hits.push((shelf_chip, Hit::ShelfOpenChip));
    }
}

fn draw_hud(app: &mut LauncherLab, p: &Painter, rect: Rect) {
    let Some(g) = &app.active else {
        return;
    };
    let w = rect.width();
    let h = rect.height();
    let line = format!("{} - {}", g.consumer.name(), g.consumer.reason());
    let size = w * 0.030;
    let pill = Rect::from_center_size(
        pos2(rect.center().x, rect.top() + h * 0.068),
        vec2(est_w(&line, size) + w * 0.06, h * 0.034),
    );
    p.rect_filled(pill, CornerRadius::same((pill.height() * 0.5) as u8), theme::surface_hi());
    p.text(
        pill.center(),
        Align2::CENTER_CENTER,
        line,
        FontId::proportional(size),
        theme::accent(),
    );
}

fn draw_assistant(app: &mut LauncherLab, p: &Painter, rect: Rect) {
    let w = rect.width();
    let h = rect.height();
    let alpha = app.assistant_ttl.min(1.0);
    let bubble = Rect::from_min_size(
        pos2(rect.left() + w * 0.05, rect.bottom() - h * 0.155),
        vec2(w * 0.60, h * 0.085),
    );
    p.rect_filled(
        bubble,
        CornerRadius::same(20),
        theme::with_alpha(theme::surface_hi(), 0.95 * alpha),
    );
    let bar = Rect::from_min_size(bubble.min, vec2(5.0, bubble.height()));
    p.rect_filled(bar, CornerRadius::same(2), theme::with_alpha(theme::accent(), alpha));
    p.text(
        pos2(bubble.left() + w * 0.05, bubble.center().y - h * 0.014),
        Align2::LEFT_CENTER,
        "Assistant ready",
        FontId::proportional(w * 0.045),
        theme::with_alpha(theme::text(), alpha),
    );
    p.text(
        pos2(bubble.left() + w * 0.05, bubble.center().y + h * 0.018),
        Align2::LEFT_CENTER,
        "AssistantTouchConsumer",
        FontId::proportional(w * 0.030),
        theme::with_alpha(theme::subtext(), alpha),
    );
    app.hits.push((bubble, Hit::AssistantOk));
}

fn draw_locked(app: &mut LauncherLab, p: &Painter, rect: Rect) {
    let w = rect.width();
    let h = rect.height();
    let t = app.lock_overlay_t;
    p.rect_filled(
        rect,
        CornerRadius::same(0),
        theme::with_alpha(Color32::BLACK, 0.62 * t),
    );
    let chip = Rect::from_center_size(rect.center(), vec2(w * 0.74, h * 0.17));
    p.rect_filled(chip, CornerRadius::same(24), theme::surface_hi());
    p.text(
        pos2(chip.center().x, chip.center().y - h * 0.028),
        Align2::CENTER_CENTER,
        "Device locked",
        FontId::proportional(w * 0.06),
        theme::text(),
    );
    p.text(
        pos2(chip.center().x, chip.center().y + h * 0.004),
        Align2::CENTER_CENTER,
        "DeviceLockedInputConsumer - gestures disabled",
        FontId::proportional(w * 0.030),
        theme::subtext(),
    );
    p.text(
        pos2(chip.center().x, chip.center().y + h * 0.038),
        Align2::CENTER_CENTER,
        "TAP TO UNLOCK",
        FontId::proportional(w * 0.036),
        theme::accent(),
    );
    app.hits.push((chip, Hit::UnlockChip));
}

fn draw_toast(p: &Painter, rect: Rect, msg: &str, ttl: f32) {
    let w = rect.width();
    let h = rect.height();
    let size = w * 0.036;
    let alpha = (ttl / 0.7).clamp(0.0, 1.0);
    let pill = Rect::from_center_size(
        pos2(rect.center().x, rect.top() + h * 0.115),
        vec2(est_w(msg, size) + w * 0.07, h * 0.048),
    );
    p.rect_filled(
        pill,
        CornerRadius::same((pill.height() * 0.5) as u8),
        theme::with_alpha(theme::surface_hi(), 0.95 * alpha),
    );
    p.text(
        pill.center(),
        Align2::CENTER_CENTER,
        msg,
        FontId::proportional(size),
        theme::with_alpha(theme::text(), alpha),
    );
}

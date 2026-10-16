//! Application state and input routing.
//!
//! Touches are routed through [`crate::model::state`] (the reconstructed
//! input-consumer state machine); rendering lives in [`crate::views`].
//!
//! Gesture map (see README):
//! * swipe left/right on home          -> next/previous page
//! * swipe right from the left edge   -> Shelf (slides in from the left)
//! * swipe up in the middle           -> app drawer
//! * swipe up from the bottom band    -> recents (overview)
//! * tap chevron / page dots          -> drawer / jump to page
//! * drawer: swipe down closes, swipe right -> Hidden Space
//! * recents: swipe up on a card dismisses it, drag across = carousel
//! * shelf: swipe left closes; tap cards = getCard; hold a memo = deleteCard
//! * hold empty home = light/dark chrome; double-tap empty home = lock demo

use eframe::egui::{self, Color32, Context, Pos2, Rect, Vec2};

use crate::clock;
use crate::icons::{self, App};
use crate::model::hidden::HiddenSpace;
use crate::model::pager::Pager;
use crate::model::shelf::{CardManager, DeleteCardResult};
use crate::model::state::{self, ConsumerType, GestureOrigin, ReleaseOutcome, Screen, Zones};
use crate::theme;
use crate::views::{self, Hit};

/// One entry of the (fake) recents stack.
#[derive(Clone, Debug)]
pub struct Task {
    pub app: App,
    pub title: String,
    pub caption: String,
    pub accent: u32,
    pub resume_t: f32,
    pub dismiss_t: f32,
    pub dying: bool,
    pub x_off: f32,
}

/// Info captured on touch-down (long-press targets and the hold-to-unlock pad).
struct PressInfo {
    pos: Pos2,
    t: f64,
    /// Memo-card tag when the press started on a shelf card.
    card: Option<String>,
    /// Hidden-item name when the press started on a hidden-space item.
    hidden_item: Option<String>,
    /// Press started on a long-pressable menu glyph (⋮).
    menu: bool,
    /// Press started on plain background (home, no hit).
    bg: bool,
    /// Press started on the Hidden Space hold pad.
    unlock: bool,
    /// The long-press action already fired for this press.
    consumed: bool,
}

/// Gesture latched at touch-down.
struct ActiveGesture {
    consumer: ConsumerType,
    _origin: GestureOrigin,
    start: Pos2,
    moved: f32,
    t: f64,
}

/// The launcher.
pub struct LauncherLab {
    /// Top-level surface.
    pub screen: Screen,
    /// Home pager (5 pages, matching `default_workspace_5x5_7.xml`).
    pub pager: Pager,
    /// Reconstructed Hidden Space model.
    pub hidden: HiddenSpace,
    /// Reconstructed shelf CardProvider store.
    pub cards: CardManager,
    /// Demo recents stack.
    pub tasks: Vec<Task>,
    pub carousel: usize,
    pub carousel_drag: f32,
    pub task_drag: Option<(usize, f32, f32)>,

    // ---------------------------------------------------------- animations
    pub drawer_t: f32,
    pub drawer_scroll: f32,
    pub shelf_t: f32,
    pub shelf_scroll: f32,
    pub shelf_content_h: f32,
    /// Latched open state of the Shelf (`shelf_t` follows it).
    shelf_open: bool,
    pub hidden_t: f32,
    pub overview_t: f32,
    pub lock_t: f32,
    pub folder_t: f32,
    /// Open folder as `(page, item index)` on that page.
    pub folder: Option<(usize, usize)>,

    // ------------------------------------------------------------ demo bits
    /// Device-locked overlay (double-tap empty home toggles it).
    pub locked_ui: bool,
    /// `true` = dark chrome (OxygenOS 10 default), `false` = light chrome.
    pub theme_dark: bool,
    pub toast: Option<(String, f32)>,
    pub time: clock::TimeParts,
    /// Monotonic input time (seconds) of the current frame.
    now: f64,
    /// Hit rectangles rebuilt during painting, in paint (z) order.
    pub hits: Vec<(Rect, Hit)>,
    pub unlock_hold: f32,
    press: Option<PressInfo>,
    active: Option<ActiveGesture>,
    last_pos: Pos2,
    last_tap: Option<(f64, Pos2)>,
    memo_seq: u32,
    drawer_hint_shown: bool,
}

impl LauncherLab {
    /// Build the launcher and seed the demo CardProvider channels.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::BLACK;
        cc.egui_ctx.set_visuals(visuals);

        let mut cards = CardManager::new();
        let _ = cards.register_channel("weather", "demo.weather");
        let _ = cards.register_channel("health", "demo.health");
        let _ = cards.register_channel("music", "demo.music");
        let _ = cards.register_channel("memo", "demo.memo");
        let _ = cards.post_card(
            "weather",
            "today",
            "Weather",
            "Cloudy with sunny intervals. High 84\u{00B0}, low 71\u{00B0}. Light breeze from the north-west.",
            0x17B9E8,
        );
        let _ = cards.post_card(
            "music",
            "now",
            "Now playing",
            "Nothing playing. Open Music to start a playlist - the shelf card updates through postCard().",
            0xE8452C,
        );
        let _ = cards.post_card(
            "health",
            "steps",
            "Steps",
            "6,204 steps today. Goal 8,000 - 1,796 to go. Card posted by the Health channel.",
            0x1F9C6C,
        );

        let tasks = [App::Messages, App::Chrome, App::Gallery, App::Music]
            .into_iter()
            .map(|app| Task {
                app,
                title: app.name().to_string(),
                caption: "Opened from Home".to_string(),
                accent: app.accent(),
                resume_t: 0.0,
                dismiss_t: 0.0,
                dying: false,
                x_off: 0.0,
            })
            .collect();

        Self {
            screen: Screen::Home,
            pager: Pager::new(5),
            hidden: HiddenSpace::new(),
            cards,
            tasks,
            carousel: 0,
            carousel_drag: 0.0,
            task_drag: None,
            drawer_t: 0.0,
            drawer_scroll: 0.0,
            shelf_t: 0.0,
            shelf_scroll: 0.0,
            shelf_content_h: 0.0,
            shelf_open: false,
            hidden_t: 0.0,
            overview_t: 0.0,
            lock_t: 0.0,
            folder_t: 0.0,
            folder: None,
            locked_ui: false,
            theme_dark: true,
            toast: Some(("Hold empty home: light/dark chrome".into(), 2.6)),
            time: clock::now(),
            now: 0.0,
            hits: Vec::new(),
            unlock_hold: 0.0,
            press: None,
            active: None,
            last_pos: Pos2::ZERO,
            last_tap: None,
            memo_seq: 0,
            drawer_hint_shown: false,
        }
    }

    // ------------------------------------------------------------- helpers

    fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), 2.2));
    }

    /// Topmost allowed hit at `pos`.
    fn hit_at(&self, pos: Pos2) -> Option<Hit> {
        self.hits
            .iter()
            .rev()
            .find(|(r, h)| r.contains(pos) && self.hit_allowed(h))
            .map(|(_, h)| h.clone())
    }

    /// Only hits belonging to the current surface are live.
    fn hit_allowed(&self, h: &Hit) -> bool {
        if self.locked_ui {
            return matches!(h, Hit::UnlockTap);
        }
        match self.screen {
            Screen::Hidden => {
                if !self.hidden.is_unlocked() {
                    return matches!(h, Hit::HiddenHold);
                }
                matches!(
                    h,
                    Hit::HiddenClose | Hit::HiddenAdd | Hit::HiddenMenu | Hit::HiddenItem(_)
                )
            }
            Screen::Drawer => matches!(h, Hit::DrawerApp(_) | Hit::DrawerSearch | Hit::DrawerMenu),
            Screen::Overview => matches!(h, Hit::Task(_) | Hit::TaskIcon(_) | Hit::TaskClearAll),
            Screen::Home => {
                if self.folder.is_some() {
                    return matches!(h, Hit::FolderClose | Hit::FolderItem(_));
                }
                if self.shelf_open_or_visible() {
                    return matches!(
                        h,
                        Hit::ShelfTool(_)
                            | Hit::ShelfApp(_)
                            | Hit::ShelfAction
                            | Hit::ShelfNewNote
                            | Hit::ShelfCard(_)
                    );
                }
                true
            }
        }
    }

    fn shelf_open_or_visible(&self) -> bool {
        self.shelf_open
    }

    fn open_drawer_hint(&mut self) {
        if !self.drawer_hint_shown {
            self.drawer_hint_shown = true;
            self.toast("Swipe right in the drawer for Hidden Space");
        }
    }

    fn launch(&mut self, app: App) {
        let name = app.name().to_string();
        self.tasks.insert(
            0,
            Task {
                app,
                title: name.clone(),
                caption: "Opened from Home".to_string(),
                accent: app.accent(),
                resume_t: 1.0,
                dismiss_t: 0.0,
                dying: false,
                x_off: 0.0,
            },
        );
        self.tasks.truncate(6);
        self.carousel = 0;
        self.toast(format!("Launching {name} (demo)"));
    }

    fn dismiss_task(&mut self, i: usize) {
        if let Some(t) = self.tasks.get_mut(i) {
            t.dying = true;
        }
    }

    fn long_press(
        &mut self,
        card: Option<String>,
        hidden_item: Option<String>,
        menu: bool,
        bg: bool,
    ) {
        if let Some(tag) = card {
            if self.cards.delete_card("memo", &tag) == DeleteCardResult::Deleted {
                self.toast("Memo card deleted (deleteCard)");
            } else {
                self.toast("Card is publisher-owned (getCard only)");
            }
        } else if let Some(name) = hidden_item {
            self.hidden.unhide_item(&name);
            self.toast(format!("Unhidden {name} (demo)"));
        } else if menu {
            let on = !self.hidden.password_enabled();
            self.hidden.set_password_enabled(on);
            if on {
                self.toast("Hidden Space password ON - hold the ring");
            } else {
                self.toast("Hidden Space password OFF (demo)");
            }
        } else if bg && self.screen == Screen::Home {
            self.theme_dark = !self.theme_dark;
            let s = if self.theme_dark { "on" } else { "off" };
            self.toast(format!("Dark chrome: {s}"));
        }
    }

    // -------------------------------------------------------------- input

    fn begin(&mut self, pos: Pos2, time: f64, screen: Rect) {
        let hit = self.hit_at(pos);
        let mut pr = PressInfo {
            pos,
            t: time,
            card: None,
            hidden_item: None,
            menu: false,
            bg: hit.is_none(),
            unlock: false,
            consumed: false,
        };
        match &hit {
            Some(Hit::ShelfCard(i)) => {
                if let Some(card) = self.cards.cards().get(*i) {
                    if card.channel == "memo" {
                        pr.card = Some(card.tag.clone());
                    }
                }
            }
            Some(Hit::HiddenItem(i)) => {
                if let Some(it) = self.hidden.items().get(*i) {
                    pr.hidden_item = Some(it.name.clone());
                }
            }
            Some(Hit::HiddenMenu) | Some(Hit::DrawerMenu) => pr.menu = true,
            Some(Hit::HiddenHold) => pr.unlock = true,
            _ => {}
        }
        let (consumer, origin) = state::classify_start(
            self.screen,
            self.locked_ui,
            pos.x - screen.left(),
            pos.y - screen.top(),
            screen.width(),
            screen.height(),
            Zones::default(),
        );
        self.active = Some(ActiveGesture {
            consumer,
            _origin: origin,
            start: pos,
            moved: 0.0,
            t: time,
        });
        self.last_pos = pos;
        self.press = Some(pr);
        self.unlock_hold = 0.0;
    }

    fn drag(&mut self, pos: Pos2, _time: f64, screen: Rect) {
        let Some(g) = &self.active else {
            self.last_pos = pos;
            return;
        };
        let consumer = g.consumer;
        let start = g.start;
        let d = pos - self.last_pos;
        self.last_pos = pos;
        let tdx = pos.x - start.x;
        let tdy = pos.y - start.y;
        if let Some(g) = &mut self.active {
            g.moved += d.length();
        }
        self.drive(consumer, d, tdx, tdy, screen);
    }

    fn drive(&mut self, consumer: ConsumerType, d: Vec2, tdx: f32, tdy: f32, screen: Rect) {
        let w = screen.width();
        let h = screen.height();

        if self.locked_ui {
            return;
        }
        if let Some(pr) = &self.press {
            if pr.unlock {
                return;
            }
        }

        // Shelf overlay (full-screen, above home).
        if self.screen == Screen::Home && (self.shelf_open || self.shelf_t > 0.02) {
            if tdy.abs() > tdx.abs() {
                let max = (self.shelf_content_h - h + 60.0).max(0.0);
                self.shelf_scroll = (self.shelf_scroll - d.y).clamp(0.0, max);
            } else if self.shelf_open {
                // Latched open: left drags pull the panel away again.
                if tdx < -4.0 {
                    self.shelf_t = (1.0 + tdx / (w * 0.55)).clamp(0.0, 1.0);
                }
            } else {
                // Still sliding in/out with the finger.
                self.shelf_t = (tdx / (w * 0.55)).clamp(0.0, 1.0);
            }
            return;
        }

        match self.screen {
            Screen::Drawer => {
                let started_low = self
                    .press
                    .as_ref()
                    .map(|p| p.pos.y > screen.top() + 118.0)
                    .unwrap_or(false);
                let rows = icons::DRAWER.len().div_ceil(5);
                let content_h = rows as f32 * 96.0 + 40.0;
                let view_h = (h - 118.0).max(1.0);
                let max = (content_h - view_h).max(0.0);
                let want_close = tdy > 0.0 && self.drawer_scroll <= 0.5;
                if started_low && !want_close && tdy.abs() > tdx.abs() {
                    self.drawer_scroll = (self.drawer_scroll - d.y).clamp(0.0, max);
                } else if tdy > 0.0 && tdy.abs() > tdx.abs() {
                    self.drawer_t = (1.0 + tdy / (h * 0.45)).clamp(0.0, 1.0);
                }
            }
            Screen::Hidden => {}
            Screen::Overview => {
                let task = self.press_task();
                if tdx.abs() > tdy.abs() * 1.15 {
                    self.task_drag = None;
                    self.carousel_drag = tdx;
                } else if let Some(ti) = task {
                    if tdy < -2.0 {
                        self.carousel_drag = 0.0;
                        self.task_drag = Some((ti, tdx, tdy));
                    }
                }
            }
            Screen::Home => match consumer {
                ConsumerType::Shelf => {
                    self.shelf_t = (tdx / (w * 0.55)).clamp(0.0, 1.0);
                }
                ConsumerType::Pager => {
                    if self.pager.current == 0 && tdx > 4.0 && tdx.abs() > tdy.abs() {
                        // Right-drag on the first page pulls the Shelf instead.
                        self.shelf_t = (tdx / (w * 0.55)).clamp(0.0, 1.0);
                    } else if tdy < -6.0 && tdy.abs() > tdx.abs() && self.pager.offset == 0.0 {
                        self.drawer_t = (-tdy / (h * 0.40)).clamp(0.0, 1.0);
                    } else {
                        self.pager.drag(d.x, w);
                    }
                }
                ConsumerType::Overview => {
                    self.overview_t = (-tdy / (h * 0.35)).clamp(0.0, 1.0);
                }
                _ => {}
            },
        }
    }

    /// Recents card index latched at press-down (for swipe-up dismissal).
    fn press_task(&self) -> Option<usize> {
        let pr = self.press.as_ref()?;
        self.hits.iter().rev().find_map(|(r, h)| {
            if r.contains(pr.pos) {
                if let Hit::Task(i) = h {
                    return Some(*i);
                }
            }
            None
        })
    }

    fn finish(&mut self, pos: Pos2, time: f64, vel: Vec2, screen: Rect) {
        let Some(g) = self.active.take() else {
            self.press = None;
            return;
        };
        let pr = self.press.take();
        let dx = pos.x - g.start.x;
        let dy = pos.y - g.start.y;
        let tap = g.moved < 14.0 && (time - g.t) < 0.35;
        let consumed = pr.as_ref().map(|p| p.consumed).unwrap_or(false);
        let unlock = pr.as_ref().map(|p| p.unlock).unwrap_or(false);
        self.unlock_hold = 0.0;

        if unlock {
            // Released before the ring completed; nothing to do.
            return;
        }
        if consumed {
            return;
        }

        let w = screen.width();
        let h = screen.height();

        // Shelf overlay first: it covers the home surface.
        if self.screen == Screen::Home && (self.shelf_open || self.shelf_t > 0.02) {
            if dx < -w * 0.12 {
                self.shelf_open = false;
                self.shelf_t = 0.0;
                return;
            }
            if tap {
                self.handle_tap(pos, time);
                return;
            }
            // A sideways drag that ended far enough (or flicked) latches the
            // shelf open; vertical drags were scrolls.
            if self.shelf_t >= 0.45 || dx > w * 0.12 {
                self.shelf_open = true;
            }
            return;
        }

        if tap {
            self.handle_tap(pos, time);
            return;
        }

        match self.screen {
            Screen::Drawer => match state::classify_release(
                g.consumer,
                self.screen,
                dx,
                dy,
                vel.x,
                vel.y,
                w,
                h,
            ) {
                ReleaseOutcome::OpenHidden => {
                    self.pager.offset = 0.0;
                    self.screen = Screen::Hidden;
                    self.toast("Hidden Space (demo)");
                }
                ReleaseOutcome::GoHome => {
                    self.screen = Screen::Home;
                }
                _ => {
                    if self.drawer_t < 0.55 {
                        self.screen = Screen::Home;
                    }
                }
            },
            Screen::Overview => {
                if let Some((ti, _, tdy)) = self.task_drag.take() {
                    if tdy < -h * 0.10 || vel.y < -700.0 {
                        self.dismiss_task(ti);
                    }
                }
                if self.carousel_drag < -w * 0.18 {
                    self.carousel = (self.carousel + 1).min(self.tasks.len().saturating_sub(1));
                } else if self.carousel_drag > w * 0.18 {
                    self.carousel = self.carousel.saturating_sub(1);
                }
                self.carousel_drag = 0.0;
                if let ReleaseOutcome::GoHome = state::classify_release(
                    g.consumer,
                    self.screen,
                    dx,
                    dy,
                    vel.x,
                    vel.y,
                    w,
                    h,
                ) {
                    self.screen = Screen::Home;
                }
            }
            Screen::Home => match g.consumer {
                ConsumerType::Overview => {
                    if let ReleaseOutcome::ShowOverview = state::classify_release(
                        g.consumer,
                        self.screen,
                        dx,
                        dy,
                        vel.x,
                        vel.y,
                        w,
                        h,
                    ) {
                        self.screen = Screen::Overview;
                        self.carousel = self.carousel.min(self.tasks.len().saturating_sub(1));
                    }
                }
                ConsumerType::Shelf => {
                    if let ReleaseOutcome::OpenShelf = state::classify_release(
                        g.consumer,
                        self.screen,
                        dx,
                        dy,
                        vel.x,
                        vel.y,
                        w,
                        h,
                    ) {
                        self.shelf_scroll = 0.0;
                        self.shelf_open = true;
                    } else {
                        self.shelf_t = 0.0;
                    }
                }
                ConsumerType::Pager => {
                    if -dy > h * 0.12 || -vel.y > 650.0 {
                        self.screen = Screen::Drawer;
                        self.open_drawer_hint();
                    } else if self.shelf_t > 0.4 {
                        self.shelf_scroll = 0.0;
                        self.shelf_open = true;
                    } else {
                        self.pager.settle(vel.x, w);
                    }
                }
                _ => {}
            },
            Screen::Hidden => {}
        }
    }

    fn handle_tap(&mut self, pos: Pos2, time: f64) {
        let hit = self.hit_at(pos);
        match hit {
            Some(Hit::Pill) => self.toast("Google Search (demo)"),
            Some(Hit::PageDot(i)) => {
                self.pager.current = i.min(self.pager.pages.saturating_sub(1));
                self.pager.offset = 0.0;
            }
            Some(Hit::Caret) => {
                self.screen = Screen::Drawer;
                self.open_drawer_hint();
            }
            Some(Hit::HomeItem { page, idx }) => match views::item_at(page, idx) {
                Some(views::HomeItem::App(a)) => self.launch(a),
                Some(views::HomeItem::Folder { .. }) => {
                    self.folder = Some((page, idx));
                }
                None => {}
            },
            Some(Hit::Dock(i)) => {
                if let Some(a) = icons::DOCK.get(i) {
                    self.launch(*a);
                }
            }
            Some(Hit::FolderClose) => self.folder = None,
            Some(Hit::FolderItem(i)) => {
                if let Some((page, idx)) = self.folder {
                    if let Some(views::HomeItem::Folder { apps, .. }) = views::item_at(page, idx) {
                        if let Some(a) = apps.get(i) {
                            self.launch(*a);
                        }
                    }
                }
                self.folder = None;
            }
            Some(Hit::DrawerApp(i)) => {
                if let Some(a) = icons::DRAWER.get(i) {
                    self.launch(*a);
                }
                self.screen = Screen::Home;
            }
            Some(Hit::DrawerSearch) => self.toast("Search apps (demo)"),
            Some(Hit::DrawerMenu) => self.toast("Hold this glyph: Hidden Space password"),
            Some(Hit::HiddenClose) => self.screen = Screen::Drawer,
            Some(Hit::HiddenAdd) => self.toast("Add apps to Hidden Space (demo)"),
            Some(Hit::HiddenMenu) => self.toast("Hidden Space options"),
            Some(Hit::HiddenItem(i)) => {
                if let Some(it) = self.hidden.items().get(i) {
                    let n = it.name.clone();
                    self.toast(format!("Open {n} (demo)"));
                }
            }
            Some(Hit::HiddenHold) => {}
            Some(Hit::ShelfTool(i)) => {
                const NAMES: [&str; 5] = ["Calculator", "Selfie", "New timer", "Send files", "New tab"];
                if let Some(n) = NAMES.get(i) {
                    self.toast(format!("Toolbox: {n} (demo)"));
                }
            }
            Some(Hit::ShelfApp(i)) => {
                if let Some(a) = icons::SHELF_RECENTS.get(i) {
                    self.launch(*a);
                }
            }
            Some(Hit::ShelfAction) => self.toast("Parking Location: action (demo)"),
            Some(Hit::ShelfNewNote) => {
                self.memo_seq += 1;
                let tag = format!("memo-{}", self.memo_seq);
                let body = format!("Reconstructed CardProvider note #{}.", self.memo_seq);
                let _ = self.cards.post_card("memo", &tag, "Memo", &body, 0xFFB300);
                self.toast("postCard(memo) (demo)");
            }
            Some(Hit::ShelfCard(i)) => {
                if let Some(card) = self.cards.cards().get(i) {
                    let hyphen = if card.channel == "memo" { " - hold to delete" } else { "" };
                    self.toast(format!("getCard({}, {}){hyphen}", card.channel, card.tag));
                }
            }
            Some(Hit::Task(i)) => {
                if let Some(t) = self.tasks.get(i) {
                    let n = t.title.clone();
                    self.carousel = i;
                    self.toast(format!("Resume {n} (demo)"));
                }
            }
            Some(Hit::TaskIcon(i)) => {
                if let Some(t) = self.tasks.get(i) {
                    let n = t.title.clone();
                    self.carousel = i;
                    self.toast(format!("Switch to {n} (demo)"));
                }
            }
            Some(Hit::TaskClearAll) => {
                self.tasks.clear();
                self.carousel = 0;
                self.toast("Cleared all tasks (demo)");
            }
            Some(Hit::UnlockTap) => {
                self.locked_ui = false;
                self.toast("Unlocked (demo)");
            }
            None => {
                if self.screen == Screen::Home && self.folder.is_none() && !self.shelf_open {
                    if let Some((t0, p0)) = self.last_tap {
                        if time - t0 < 0.35 && (pos - p0).length() < 56.0 {
                            self.locked_ui = true;
                            self.toast("Device locked (demo) - tap to unlock");
                            self.last_tap = None;
                            return;
                        }
                    }
                    self.last_tap = Some((time, pos));
                }
            }
        }
    }

    // -------------------------------------------------------------- update

    fn tick(&mut self, dt: f32, screen: Rect) {
        self.time = clock::now();
        let h = screen.height();

        let drawer_target = if self.screen == Screen::Drawer { 1.0 } else { 0.0 };
        let overview_target = if self.screen == Screen::Overview { 1.0 } else { 0.0 };
        let hidden_target = if self.screen == Screen::Hidden { 1.0 } else { 0.0 };
        let lock_target = if self.locked_ui { 1.0 } else { 0.0 };
        let folder_target = if self.folder.is_some() { 1.0 } else { 0.0 };

        if self.active.is_none() {
            self.drawer_t = theme::approach(self.drawer_t, drawer_target, dt, 14.0);
            self.overview_t = theme::approach(self.overview_t, overview_target, dt, 14.0);
            self.lock_t = theme::approach(self.lock_t, lock_target, dt, 16.0);
            // Pager offset settles when no gesture is live.
            if self.pager.offset != 0.0 {
                self.pager.offset = theme::approach(self.pager.offset, 0.0, dt, 18.0);
                if self.pager.offset.abs() < 0.5 {
                    self.pager.offset = 0.0;
                }
            }
            self.task_drag = None;
        } else {
            self.lock_t = theme::approach(self.lock_t, lock_target, dt, 16.0);
        }
        self.hidden_t = theme::approach(self.hidden_t, hidden_target, dt, 14.0);
        self.folder_t = theme::approach(self.folder_t, folder_target, dt, 18.0);

        // Shelf slides open/closed (except while dragging it).
        if self.active.is_none() {
            let target = if self.shelf_open { 1.0 } else { 0.0 };
            self.shelf_t = theme::approach(self.shelf_t, target, dt, 14.0);
        }
        let max_shelf = (self.shelf_content_h - h + 60.0).max(0.0);
        self.shelf_scroll = self.shelf_scroll.clamp(0.0, max_shelf);

        // Carousel snap.
        if self.task_drag.is_none() {
            self.carousel_drag = theme::approach(self.carousel_drag, 0.0, dt, 16.0);
            if self.carousel_drag.abs() < 0.5 {
                self.carousel_drag = 0.0;
            }
        }

        // Task animations.
        for t in &mut self.tasks {
            if t.resume_t > 0.0 {
                t.resume_t = (t.resume_t - dt * 3.0).max(0.0);
            }
            if t.dying {
                t.dismiss_t = (t.dismiss_t + dt * 4.0).min(1.0);
            }
        }
        self.tasks.retain(|t| !(t.dying && t.dismiss_t >= 1.0));
        if self.tasks.is_empty() {
            self.carousel = 0;
        } else {
            self.carousel = self.carousel.min(self.tasks.len() - 1);
        }

        // Toast lifetime.
        let mut clear = false;
        if let Some((_, ttl)) = self.toast.as_mut() {
            *ttl -= dt;
            clear = *ttl <= 0.0;
        }
        if clear {
            self.toast = None;
        }

        // Long-press (0.65 s) on the latching targets.
        if let Some(pr) = &self.press {
            if !pr.consumed && self.now - pr.t > 0.65 {
                let card = pr.card.clone();
                let hidden_item = pr.hidden_item.clone();
                let menu = pr.menu;
                let bg = pr.bg;
                if let Some(p) = &mut self.press {
                    p.consumed = true;
                }
                self.long_press(card, hidden_item, menu, bg);
            }
        }

        // Hold-to-unlock ring on the Hidden Space gate.
        let holding = self
            .press
            .as_ref()
            .map(|p| p.unlock && !p.consumed)
            .unwrap_or(false);
        if holding {
            self.unlock_hold += dt;
            if self.unlock_hold >= 0.65 {
                self.unlock_hold = 0.0;
                self.hidden.unlock();
                self.toast("Hidden Space unlocked");
            }
        } else if self.active.is_none() {
            self.unlock_hold = 0.0;
        }
    }

}

impl eframe::App for LauncherLab {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        let screen = ctx.screen_rect();
        let time = ctx.input(|i| i.time);
        let pos = ctx
            .input(|i| i.pointer.latest_pos())
            .unwrap_or_else(|| screen.center());
        let vel = ctx.input(|i| i.pointer.velocity());
        let pressed = ctx.input(|i| i.pointer.any_pressed());
        let released = ctx.input(|i| i.pointer.any_released());
        let down = ctx.input(|i| i.pointer.any_down());
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        self.now = time;

        if pressed {
            self.begin(pos, time, screen);
        }
        if down && self.active.is_some() {
            self.drag(pos, time, screen);
        }
        if released {
            self.finish(pos, time, vel, screen);
        }
        self.tick(dt, screen);

        let painter = ctx.layer_painter(egui::LayerId::background());
        let rect = Rect::from_min_size(Pos2::ZERO, screen.size());
        views::paint(self, &painter, rect);

        ctx.request_repaint();
    }
}

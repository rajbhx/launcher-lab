//! Application state and input routing.
//!
//! Touches are routed through [`crate::model::state`] (the reconstructed
//! input-consumer state machine); rendering lives in [`crate::views`].

use eframe::egui::{self, Context, Pos2, Rect, Vec2};

use crate::model::hidden::HiddenSpace;
use crate::model::pager::Pager;
use crate::model::shelf::{CardManager, DeleteCardResult, PostCardResult};
use crate::model::state::{self, ConsumerType, ReleaseOutcome, Screen, Zones};
use crate::theme;
use crate::views::{self, Hit};

/// One home-screen app tile (demo data only — nothing is really launched).
#[derive(Clone, Debug)]
pub struct AppTile {
    pub name: String,
    pub accent: u32,
}

/// One entry of the (fake) recents stack.
#[derive(Clone, Debug)]
pub struct Task {
    pub title: String,
    pub caption: String,
    pub accent: u32,
    pub resume_t: f32,
    pub dismiss_t: f32,
    pub dying: bool,
    pub x_off: f32,
}

/// Info captured on touch-down.
pub struct PressInfo {
    pub pos: Pos2,
    pub t: f64,
    /// Shelf card tag under the press (for long-press delete).
    pub card: Option<String>,
    /// Press started on the hold-to-unlock pad.
    pub unlock: bool,
    /// Recents task under the press (for horizontal dismiss).
    pub task: Option<usize>,
    /// Long-press already consumed for this press.
    pub consumed: bool,
}

/// The gesture currently in flight.
pub struct ActiveGesture {
    pub consumer: ConsumerType,
    pub start: Pos2,
    pub last: Pos2,
    pub moved: f32,
}

/// Snapshot of the frame's input, read once per update.
pub struct FrameInput {
    pub time: f64,
    pub pos: Option<Pos2>,
    pub down: bool,
    pub pressed: bool,
    pub released: bool,
    pub vel: Vec2,
    pub zoom: f32,
    pub touches: usize,
    pub screen: Rect,
}

pub struct LauncherLab {
    // surfaces
    pub screen: Screen,
    pub locked_ui: bool,
    pub shelf_open: bool,

    // animations (0..=1)
    pub overview_t: f32,
    pub shelf_t: f32,
    pub hidden_t: f32,
    pub lock_overlay_t: f32,
    pub assistant_ttl: f32,

    // model
    pub pager: Pager,
    pub hidden: HiddenSpace,
    pub cards: CardManager,
    pub apps: Vec<AppTile>,
    pub tasks: Vec<Task>,

    // input
    pub press: Option<PressInfo>,
    pub active: Option<ActiveGesture>,
    pub task_drag: Option<(usize, f32)>,
    pub unlock_hold: f32,
    pub pinch_accum: f32,

    // decorations
    pub memo_seq: u32,
    pub toast: Option<(String, f32)>,
    pub hits: Vec<(Rect, Hit)>,
    pub frames: u64,
}

impl LauncherLab {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        // Seed the shelf exactly the way the analyzed provider works:
        // publishers register a channel, then post cards keyed by (channel, tag).
        let mut cards = CardManager::new();
        let _ = cards.register_channel("weather", "seed");
        let _ = cards.register_channel("health", "seed");
        let _ = cards.register_channel("music", "seed");
        let _ = cards.register_channel("memo", "seed");
        let _ = cards.post_card("weather", "home", "Weather", "23 C - Sunny", 0x2E9DF7);
        let _ = cards.post_card("health", "steps", "Steps", "8,432 today - 6.1 km", 0x37B26C);
        let _ = cards.post_card(
            "music",
            "now",
            "Music",
            "Midnight Drive - Demo Artist",
            0xE8452C,
        );

        let apps = [
            ("Phone", 0x37B26C),
            ("Messages", 0x2E9DF7),
            ("Camera", 0xE8A33D),
            ("Browser", 0x3D6BE8),
            ("Gallery", 0xB85CE8),
            ("Files", 0x6B7B8C),
            ("Music", 0xE8452C),
            ("Weather", 0x2E9DF7),
            ("Notes", 0xE8C63D),
            ("Clock", 0x37A8A0),
            ("Calculator", 0x8C93A8),
            ("Settings", 0x5C6675),
            ("Maps", 0x37B26C),
            ("Mail", 0xE85C7A),
            ("Store", 0x4CC3E0),
            ("Recorder", 0xB85C5C),
        ]
        .into_iter()
        .map(|(name, accent)| AppTile {
            name: name.to_string(),
            accent,
        })
        .collect();

        let tasks = vec![
            mk_task("Messages", "3 conversations", 0x2E9DF7),
            mk_task("Browser", "product spec sheet", 0x3D6BE8),
            mk_task("Music", "Now playing", 0xE8452C),
            mk_task("Gallery", "Camera roll", 0xB85CE8),
        ];

        Self {
            screen: Screen::Home,
            locked_ui: false,
            shelf_open: false,
            overview_t: 0.0,
            shelf_t: 0.0,
            hidden_t: 0.0,
            lock_overlay_t: 0.0,
            assistant_ttl: 0.0,
            pager: Pager::new(2),
            hidden: HiddenSpace::new(),
            cards,
            apps,
            tasks,
            press: None,
            active: None,
            task_drag: None,
            unlock_hold: 0.0,
            pinch_accum: 1.0,
            memo_seq: 0,
            toast: None,
            hits: Vec::new(),
            frames: 0,
        }
    }

    pub fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), 1.8));
    }

    // ---------------------------------------------------------------- input

    fn hit_at(&self, pos: Pos2) -> Option<&Hit> {
        self.hits
            .iter()
            .rev()
            .find(|(r, h)| r.contains(pos) && self.hit_allowed(h))
            .map(|(_, h)| h)
    }

    /// Which hit targets are reachable in the current state.
    fn hit_allowed(&self, hit: &Hit) -> bool {
        let is_hidden_hit = matches!(
            hit,
            Hit::HiddenClose | Hit::PasswordToggle | Hit::UnlockHold
        );
        if self.hidden_t > 0.05 {
            return is_hidden_hit;
        }
        if self.locked_ui {
            return matches!(hit, Hit::UnlockChip);
        }
        let is_shelf_hit = matches!(hit, Hit::CloseShelf | Hit::MemoAdd | Hit::ShelfCard(_));
        if self.shelf_t > 0.5 {
            return is_shelf_hit;
        }
        !is_hidden_hit
    }

    fn card_at(&self, pos: Pos2) -> Option<String> {
        match self.hit_at(pos) {
            Some(Hit::ShelfCard(tag)) => Some(tag.clone()),
            _ => None,
        }
    }

    fn task_at(&self, pos: Pos2) -> Option<usize> {
        match self.hit_at(pos) {
            Some(Hit::Task(i)) => Some(*i),
            _ => None,
        }
    }

    fn press_on_unlock(&self, pos: Pos2) -> bool {
        matches!(self.hit_at(pos), Some(Hit::UnlockHold))
    }

    fn tick(&mut self, dt: f32, inp: &FrameInput) {
        self.frames = self.frames.wrapping_add(1);

        // --- animations ---------------------------------------------------
        let overview_target = if self.screen == Screen::Overview { 1.0 } else { 0.0 };
        let hidden_target = if self.screen == Screen::Hidden { 1.0 } else { 0.0 };
        let shelf_target = if self.shelf_open { 1.0 } else { 0.0 };
        let lock_target = if self.locked_ui { 1.0 } else { 0.0 };

        if self.active.is_none() {
            self.overview_t = theme::approach(self.overview_t, overview_target, dt, 14.0);
            self.shelf_t = theme::approach(self.shelf_t, shelf_target, dt, 14.0);
            self.pager.offset = theme::approach(self.pager.offset, 0.0, dt, 16.0);
        }
        self.hidden_t = theme::approach(self.hidden_t, hidden_target, dt, 12.0);
        self.lock_overlay_t = theme::approach(self.lock_overlay_t, lock_target, dt, 14.0);

        self.assistant_ttl = (self.assistant_ttl - dt).max(0.0);

        for t in &mut self.tasks {
            t.resume_t = theme::approach(t.resume_t, 0.0, dt, 8.0);
            let dismiss_target = if t.dying { 1.0 } else { 0.0 };
            t.dismiss_t = theme::approach(t.dismiss_t, dismiss_target, dt, 8.0);
            if !t.dying {
                t.x_off = theme::approach(t.x_off, 0.0, dt, 14.0);
            }
        }
        self.tasks
            .retain(|t| !(t.dying && t.dismiss_t > 0.96));

        if let Some((_, ttl)) = &mut self.toast {
            *ttl -= dt;
            if *ttl <= 0.0 {
                self.toast = None;
            }
        }

        // --- pinch: out on home opens Hidden Space, in inside it closes ----
        if inp.touches >= 2 {
            self.pinch_accum *= inp.zoom;
            if self.screen == Screen::Home
                && !self.locked_ui
                && self.shelf_t < 0.2
                && self.pinch_accum > 1.25
            {
                self.pinch_accum = 1.0;
                self.screen = Screen::Hidden;
                self.unlock_hold = 0.0;
                self.toast("pinch-out -> Hidden Space");
            } else if self.screen == Screen::Hidden && self.pinch_accum < 0.8 {
                self.pinch_accum = 1.0;
                self.screen = Screen::Home;
            }
        } else {
            self.pinch_accum = 1.0;
        }

        let w = inp.screen.width();
        let h = inp.screen.height();

        // --- press ---------------------------------------------------------
        if inp.pressed {
            if let Some(pos) = inp.pos {
                let (consumer, _origin) = state::classify_start(
                    self.screen,
                    self.locked_ui,
                    pos.x,
                    pos.y,
                    w,
                    h,
                    Zones::default(),
                );
                if consumer == ConsumerType::Assistant {
                    self.assistant_ttl = 2.6;
                }
                self.active = Some(ActiveGesture {
                    consumer,
                    start: pos,
                    last: pos,
                    moved: 0.0,
                });
                let card = self.card_at(pos);
                let task = self.task_at(pos);
                let unlock = self.press_on_unlock(pos);
                self.press = Some(PressInfo {
                    pos,
                    t: inp.time,
                    card,
                    unlock,
                    task,
                    consumed: false,
                });
            }
        }

        // --- drag ----------------------------------------------------------
        let mut live: Option<(ConsumerType, Pos2, Pos2, Vec2)> = None;
        if let Some(g) = &mut self.active {
            if let Some(pos) = inp.pos {
                let d = pos - g.last;
                g.moved += (d.x * d.x + d.y * d.y).sqrt();
                g.last = pos;
                live = Some((g.consumer, g.start, g.last, d));
            }
        }
        if let Some((consumer, start, last, d)) = live {
            self.drive_live(consumer, start, last, d, w, h);
        }

        // --- release -------------------------------------------------------
        if inp.released {
            let g = self.active.take();
            let pr = self.press.take();
            if let (Some(g), Some(pr)) = (g, pr) {
                if g.moved < 14.0 && (inp.time - pr.t) < 0.35 {
                    self.handle_tap(pr.pos);
                } else {
                    self.finish_gesture(&g, inp, w, h);
                }
            }
        }

        // --- hold-to-unlock (Hidden Space password pad) --------------------
        let mut unlock_now = false;
        if let Some(pr) = &self.press {
            if pr.unlock && inp.down {
                self.unlock_hold += dt;
                unlock_now = self.unlock_hold >= 0.65 && !self.hidden.is_unlocked();
            } else if !inp.down {
                self.unlock_hold = 0.0;
            }
        }
        if unlock_now {
            self.hidden.unlock();
            self.toast("Hidden Space unlocked");
        }

        // --- long-press: delete a shelf memo card --------------------------
        let mut delete_tag: Option<String> = None;
        if let Some(pr) = &self.press {
            if !pr.consumed && !pr.unlock && inp.down {
                if let Some(tag) = &pr.card {
                    if inp.time - pr.t > 0.65 {
                        delete_tag = Some(tag.clone());
                    }
                }
            }
        }
        if let Some(tag) = delete_tag {
            if let Some(pr) = &mut self.press {
                pr.consumed = true;
            }
            match self.cards.delete_card("memo", &tag) {
                DeleteCardResult::Deleted => {
                    self.toast(format!("deleteCard(memo, {tag}) -> Deleted"));
                }
                DeleteCardResult::NotFound => {
                    self.toast(format!("deleteCard(memo, {tag}) -> NotFound"));
                }
            }
        }
    }

    /// Live UI response while a finger is down.
    fn drive_live(
        &mut self,
        consumer: ConsumerType,
        start: Pos2,
        last: Pos2,
        d: Vec2,
        w: f32,
        h: f32,
    ) {
        let tdx = last.x - start.x;
        let tdy = last.y - start.y;

        // Horizontal drags on the overview move (or dismiss) a task card.
        if self.screen == Screen::Overview {
            if let Some(i) = self.press.as_ref().and_then(|p| p.task) {
                if i < self.tasks.len() && tdx.abs() > 18.0 {
                    self.task_drag = Some((i, tdx));
                }
            }
            if let Some((_, x)) = self.task_drag.as_mut() {
                *x = tdx;
                return;
            }
        }

        match consumer {
            ConsumerType::Overview => {
                self.overview_t = (-tdy / (h * 0.30)).clamp(0.0, 1.0);
            }
            ConsumerType::ResetGesture | ConsumerType::OverviewWithoutFocus => {
                self.overview_t = (1.0 - tdy / (h * 0.30)).clamp(0.0, 1.0);
            }
            ConsumerType::Shelf => {
                self.shelf_t = (tdx / (w * 0.6)).clamp(0.0, 1.0);
            }
            ConsumerType::Pager => {
                self.pager.drag(d.x, w);
            }
            _ => {}
        }
    }

    fn finish_gesture(&mut self, g: &ActiveGesture, inp: &FrameInput, w: f32, h: f32) {
        let tdx = g.last.x - g.start.x;
        let tdy = g.last.y - g.start.y;

        // Task dismissal takes precedence over the overview resize.
        if let Some((i, x_off)) = self.task_drag.take() {
            if let Some(t) = self.tasks.get_mut(i) {
                if x_off.abs() > w * 0.22 {
                    t.dying = true;
                    t.x_off = x_off;
                    let title = t.title.clone();
                    self.toast(format!("Dismissed {title}"));
                } else {
                    t.x_off = 0.0;
                }
            }
            return;
        }

        let out = state::classify_release(
            g.consumer, self.screen, tdx, tdy, inp.vel.x, inp.vel.y, w, h,
        );
        match out {
            ReleaseOutcome::Stay => {}
            ReleaseOutcome::ShowOverview => {
                self.screen = Screen::Overview;
                self.toast("OverviewInputConsumer - TYPE_OVERVIEW");
            }
            ReleaseOutcome::GoHome => {
                self.screen = Screen::Home;
                self.shelf_open = false;
                self.toast("ResetGestureInputConsumer - TYPE_RESET_GESTURE");
            }
            ReleaseOutcome::OpenShelf => {
                self.shelf_open = true;
                self.toast("shelf edge-swipe");
            }
            ReleaseOutcome::NextPage | ReleaseOutcome::PrevPage => {
                let changed = self.pager.settle(inp.vel.x, w);
                if changed {
                    self.toast(format!("page {} of {}", self.pager.current + 1, self.pager.pages));
                }
            }
        }
    }

    fn handle_tap(&mut self, pos: Pos2) {
        let hit = self.hit_at(pos).cloned();
        match hit {
            Some(Hit::LockChip) => {
                self.locked_ui = true;
                self.toast("DeviceLockedInputConsumer active - gestures blocked");
            }
            Some(Hit::UnlockChip) => {
                self.locked_ui = false;
                self.toast("unlocked");
            }
            Some(Hit::Tile(i)) => {
                if let Some(app) = self.apps.get(i) {
                    let name = app.name.clone();
                    let accent = app.accent;
                    self.tasks.insert(
                        0,
                        Task {
                            title: name.clone(),
                            caption: "Opened from Home".into(),
                            accent,
                            resume_t: 1.0,
                            dismiss_t: 0.0,
                            dying: false,
                            x_off: 0.0,
                        },
                    );
                    self.tasks.truncate(6);
                    self.toast(format!("Launching {name} - demo only"));
                }
            }
            Some(Hit::PageDot(i)) => {
                if i < self.pager.pages {
                    self.pager.current = i;
                    self.pager.offset = 0.0;
                }
            }
            Some(Hit::ShelfOpenChip) => {
                self.shelf_open = true;
            }
            Some(Hit::CloseShelf) => {
                self.shelf_open = false;
            }
            Some(Hit::MemoAdd) => {
                self.memo_seq += 1;
                let tag = format!("demo-{}", self.memo_seq);
                let body = format!("Note #{} from the shelf", self.memo_seq);
                let result = self
                    .cards
                    .post_card("memo", &tag, "Memo", &body, 0xE8C63D);
                let msg = match result {
                    PostCardResult::Posted => "Posted",
                    PostCardResult::Updated => "Updated",
                    PostCardResult::ChannelNotFound => "ChannelNotFound",
                };
                self.toast(format!("postCard(memo, {tag}) -> {msg}"));
            }
            Some(Hit::ShelfCard(tag)) => {
                if let Some(card) = self.cards.get_card("memo", &tag) {
                    self.toast(format!(
                        "getCard(memo, {tag}) -> Found id={}",
                        card.id
                    ));
                }
            }
            Some(Hit::Task(i)) => {
                if let Some(t) = self.tasks.get_mut(i) {
                    t.resume_t = 1.0;
                    let title = t.title.clone();
                    self.toast(format!("resume {title} (demo)"));
                }
            }
            Some(Hit::HomeChip) => {
                self.screen = Screen::Home;
            }
            Some(Hit::HiddenClose) => {
                self.screen = Screen::Home;
            }
            Some(Hit::PasswordToggle) => {
                let now = !self.hidden.password_enabled();
                self.hidden.set_password_enabled(now);
                let msg = if now {
                    "Hidden Space password: on - hold the pad to unlock"
                } else {
                    "Hidden Space password: off - open access"
                };
                self.toast(msg);
            }
            Some(Hit::UnlockHold) | Some(Hit::AssistantOk) | None => {}
        }
    }
}

fn mk_task(title: &str, caption: &str, accent: u32) -> Task {
    Task {
        title: title.to_string(),
        caption: caption.to_string(),
        accent,
        resume_t: 0.0,
        dismiss_t: 0.0,
        dying: false,
        x_off: 0.0,
    }
}

impl eframe::App for LauncherLab {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // The launcher is a continuously animated surface.
        ctx.request_repaint();

        let dt = ctx.input(|i| i.stable_dt).clamp(0.001, 0.05);
        let screen = ctx.screen_rect();
        let inp = ctx.input(|i| {
            let mt = i.multi_touch();
            FrameInput {
                time: i.time,
                pos: i.pointer.interact_pos(),
                down: i.pointer.primary_down(),
                pressed: i.pointer.primary_pressed(),
                released: i.pointer.primary_released(),
                vel: i.pointer.velocity(),
                zoom: mt.map(|m| m.zoom_delta).unwrap_or(1.0),
                touches: mt.map(|m| m.num_touches).unwrap_or(0),
                screen,
            }
        });

        self.tick(dt, &inp);

        egui::CentralPanel::default()
            .frame(egui::Frame {
                fill: theme::bg(),
                ..Default::default()
            })
            .show(ctx, |ui| {
                let (rect, _resp) =
                    ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
                views::paint(self, ui.painter(), rect);
            });
    }
}

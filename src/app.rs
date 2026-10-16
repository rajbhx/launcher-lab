//! Application state and input routing.
//!
//! Touches are routed through [`crate::model::state`] (the reconstructed
//! input-consumer state machine); rendering lives in [`crate::views`].
//!
//! ## Real device data
//!
//! On Android every user-visible listing is backed by the phone itself:
//!
//! * the drawer is filled from [`crate::platform::query_launcher_apps`],
//! * every tap launches the real component via [`crate::platform::launch_app`],
//! * icons and the wallpaper stream in as textures through [`crate::loader`],
//! * the shade's quick tiles read the live system state and open the real
//!   settings screens (a normal app cannot flip those radios on Android 10+),
//! * Hidden Space stores real hidden packages, not catalog names.
//!
//! The desktop dev build (no Android) keeps the bundled demo surfaces so the
//! UI can be worked on without a device; those paths are all behind
//! [`crate::platform::is_android`].
//!
//! ## Gesture map (see README)
//!
//! * swipe left/right on home          -> next/previous page
//! * swipe right from the left edge   -> Shelf (slides in from the left)
//! * swipe up in the middle           -> app drawer
//! * swipe up from the bottom band    -> recents (overview)
//! * tap chevron / page dots          -> drawer / jump to page
//! * drawer: swipe down closes, swipe right -> Hidden Space
//! * recents: swipe up on a card dismisses it, drag across = carousel
//! * shelf: swipe left closes; tap cards = getCard; hold a memo = deleteCard
//! * hold empty home = light/dark chrome; double-tap empty home = lock demo
//! * swipe down from the status bar   -> notification shade (tiles/brightness)
//! * tap any app                     -> launches the real app (device) or the
//!                                      in-app preview (desktop demo)
//! * hold an app in the drawer       -> hide it; hold it in Hidden Space to restore

use std::collections::{HashMap, VecDeque};

use eframe::egui::{self, Color32, Context, Pos2, Rect, Vec2};

use crate::clock;
use crate::icons::{self, App};
use crate::loader::{Loader, Reply, Request};
use crate::model::hidden::{merge_hidden, HiddenEntry, HiddenReal, HiddenSpace};
use crate::model::pager::Pager;
use crate::model::shelf::{CardManager, DeleteCardResult};
use crate::model::state::{self, ConsumerType, GestureOrigin, ReleaseOutcome, Screen, Zones};
use crate::platform::{self, ImageRgba, PlatformError, RealApp, SystemState};
use crate::targets;
use crate::theme;
use crate::views::{self, Hit};

/// Icon edge (physical pixels) requested for real app icons.
const ICON_TEXTURE_EDGE: u32 = 144;
/// Loader replies converted to textures per frame (keeps frames smooth).
const LOADER_REPLIES_PER_FRAME: usize = 6;
/// Retry cadence for the installed-apps query while it keeps failing.
const APPS_RETRY_SECONDS: f64 = 6.0;

/// One entry of the recents stack.
#[derive(Clone, Debug)]
pub struct Task {
    /// Reconstructed painter for this app, when one exists.
    pub mock: Option<App>,
    /// Real package/activity once the app was actually launched.
    pub package: Option<String>,
    pub activity: Option<String>,
    pub title: String,
    pub caption: String,
    pub accent: u32,
    pub resume_t: f32,
    pub dismiss_t: f32,
    pub dying: bool,
    pub x_off: f32,
}

/// One row of the reconstructed notification shade.
#[derive(Clone, Debug)]
pub struct Notif {
    pub app: App,
    pub title: String,
    pub body: String,
    pub when: String,
    pub accent: u32,
}

/// Full-screen demo preview of a "launched" app; `t` drives the reveal that
/// grows out of the tapped icon (`src`). Desktop preview only.
#[derive(Clone, Debug)]
pub struct OpenApp {
    pub app: App,
    pub t: f32,
    pub src: Rect,
}

/// A drawer row backed by a real installed app.
#[derive(Clone, Debug)]
pub struct DrawerEntry {
    pub package: String,
    pub activity: String,
    pub label: String,
    /// Reconstructed painter used until the real icon texture arrives.
    pub mock: Option<App>,
}

/// What the drawer listing currently is.
#[derive(Clone, Debug)]
pub enum DrawerState {
    /// The device query is in flight (a retry timer may be armed).
    Loading,
    /// [`LauncherLab::drawer`] is authoritative.
    Ready,
    /// The last query failed; the string is user-facing.
    Failed(String),
}

/// Icon texture lifecycle for one package.
pub enum IconSlot {
    /// Requested from the loader, texture not built yet.
    Queued,
    Ready(egui::TextureHandle),
    /// Decoding failed once; the drawn fallback icon is used.
    Failed,
}

/// Drawer listing key: a demo-catalog app (desktop only) or a real package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrawerKey {
    Mock(App),
    Real(String),
}

/// One row of the shelf's "Recent apps" section.
#[derive(Clone, Debug)]
pub struct ShelfRecent {
    pub mock: Option<App>,
    pub package: Option<String>,
    pub activity: Option<String>,
    pub label: String,
}

/// Info captured on touch-down (long-press targets and the hold-to-unlock pad).
struct PressInfo {
    pos: Pos2,
    t: f64,
    /// Memo-card tag when the press started on a shelf card.
    card: Option<String>,
    /// Hidden-space row a long-press would restore.
    hidden_entry: Option<HiddenEntry>,
    /// Drawer app a long-press would hide from the listing.
    hide_key: Option<DrawerKey>,
    /// Press started on a long-pressable menu glyph (⋮).
    menu: bool,
    /// Press started on plain background (home, no hit).
    bg: bool,
    /// Press started on the Hidden Space hold pad.
    unlock: bool,
    /// Press started in the status-bar band (pull down = shade).
    status: bool,
    /// Press started on the shade brightness slider.
    bright: bool,
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
    /// Recents stack (real launches on device).
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

    // --------------------------------------------------------- device data
    /// Real installed launcher apps (device).
    pub drawer: Vec<DrawerEntry>,
    pub drawer_state: DrawerState,
    drawer_attempt_t: f64,
    /// Real icon textures by package.
    pub icon_cache: HashMap<String, IconSlot>,
    icon_error_shown: bool,
    apps_loaded_once: bool,
    apps_error_toasted: bool,
    /// Resolved real components for the reconstructed home/dock entries.
    resolved_targets: HashMap<App, RealApp>,
    /// The live system wallpaper (device), drawn with overscan + parallax.
    pub wallpaper_tex: Option<egui::TextureHandle>,
    wallpaper_requested: bool,
    loader: Option<Loader>,
    /// Real apps hidden from the drawer.
    pub hidden_real: Vec<HiddenReal>,
    /// Drawer letter filter (`A`..`Z`), driven by the in-app letter panel.
    pub search_letter: Option<char>,
    /// Whether the letter panel is showing under the search field.
    pub search_open: bool,
    /// Live system toggle states shown by the shade (device).
    pub system: SystemState,

    // ------------------------------------------------------------ demo bits
    /// Device-locked overlay (double-tap empty home toggles it).
    pub locked_ui: bool,
    /// `true` = dark chrome (OxygenOS 10 default), `false` = light chrome.
    pub theme_dark: bool,
    pub toast: Option<(String, f32)>,
    toast_queue: VecDeque<String>,
    pub time: clock::TimeParts,
    /// Shade pull progress (0 closed, 1 open).
    pub shade_t: f32,
    /// Screen brightness 0.12..=1.0 (dims every surface).
    pub brightness: f32,
    /// Desktop-preview quick-toggle states (device reads `system` instead).
    pub tiles: [bool; 6],
    /// Demo notifications shown in the shade (desktop preview only).
    pub notifs: Vec<Notif>,
    /// Demo preview of the last "launched" app (desktop preview only).
    pub open_app: Option<OpenApp>,
    /// First-run gesture guide.
    pub onboard: bool,
    /// Drawer apps hidden into the Hidden Space (desktop catalog).
    pub hidden_apps: Vec<App>,
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
    /// Build the launcher; on a device this immediately starts the real
    /// data pipeline (installed apps, then icons, then the wallpaper).
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::BLACK;
        cc.egui_ctx.set_visuals(visuals);

        let android = platform::is_android();

        // Desktop-preview seeds. On a device these surfaces start empty and
        // fill from the phone; nothing demo is shown there.
        let mut cards = CardManager::new();
        // The memo channel exists on every platform: notes created from the
        // shelf are stored in the reconstructed CardProvider itself.
        let _ = cards.register_channel("memo", "demo.memo");
        let mut notifs: Vec<Notif> = Vec::new();
        let mut tasks: Vec<Task> = Vec::new();
        if !android {
            let _ = cards.register_channel("weather", "demo.weather");
            let _ = cards.register_channel("health", "demo.health");
            let _ = cards.register_channel("music", "demo.music");
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

            tasks = [App::Messages, App::Chrome, App::Gallery, App::Music]
                .into_iter()
                .map(|app| Task {
                    mock: Some(app),
                    package: None,
                    activity: None,
                    title: app.name().to_string(),
                    caption: "Opened from Home".to_string(),
                    accent: app.accent(),
                    resume_t: 0.0,
                    dismiss_t: 0.0,
                    dying: false,
                    x_off: 0.0,
                })
                .collect();

            notifs = vec![
                Notif {
                    app: App::Weather,
                    title: "Weather".into(),
                    body: "Cloudy with sunny intervals. High 84\u{00B0}".into(),
                    when: "now".into(),
                    accent: 0x17B9E8,
                },
                Notif {
                    app: App::Messages,
                    title: "Messages".into(),
                    body: "Aarav: Are we still on for lunch?".into(),
                    when: "2m".into(),
                    accent: 0x16B879,
                },
                Notif {
                    app: App::PlayStore,
                    title: "Play Store".into(),
                    body: "3 apps updated in the background".into(),
                    when: "1h".into(),
                    accent: 0x00A0FF,
                },
            ];
        }

        let mut hidden = HiddenSpace::new();
        if android {
            // The demo items are not real apps; a device build shows none.
            hidden.clear_items();
        }

        let loader = if android { Loader::spawn() } else { None };

        let mut lab = Self {
            screen: Screen::Home,
            pager: Pager::new(5),
            hidden,
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
            drawer: Vec::new(),
            drawer_state: DrawerState::Loading,
            drawer_attempt_t: 0.0,
            icon_cache: HashMap::new(),
            icon_error_shown: false,
            apps_loaded_once: false,
            apps_error_toasted: false,
            resolved_targets: HashMap::new(),
            wallpaper_tex: None,
            wallpaper_requested: false,
            loader,
            hidden_real: Vec::new(),
            search_letter: None,
            search_open: false,
            system: SystemState::default(),
            locked_ui: false,
            theme_dark: true,
            toast: None,
            toast_queue: VecDeque::new(),
            time: clock::now(),
            shade_t: 0.0,
            brightness: 0.88,
            tiles: [true, false, false, false, true, false],
            notifs,
            open_app: None,
            onboard: true,
            hidden_apps: Vec::new(),
            now: 0.0,
            hits: Vec::new(),
            unlock_hold: 0.0,
            press: None,
            active: None,
            last_pos: Pos2::ZERO,
            last_tap: None,
            memo_seq: 0,
            drawer_hint_shown: false,
        };

        if android {
            lab.refresh_states_if_device();
            lab.toast("Loading installed apps\u{2026}");
            lab.request_apps();
        } else {
            lab.toast = Some(("Swipe down from the status bar for the shade".into(), 2.6));
        }
        lab
    }

    // ------------------------------------------------------------- helpers

    /// Show a toast now, or queue it behind the one on screen.
    fn toast(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        if self.toast.is_none() {
            self.toast = Some((msg, 2.4));
        } else {
            if self.toast_queue.len() >= 4 {
                self.toast_queue.pop_front();
            }
            self.toast_queue.push_back(msg);
        }
    }

    /// Topmost allowed hit at `pos`, together with its rectangle.
    fn hit_at(&self, pos: Pos2) -> Option<(Rect, Hit)> {
        self.hits
            .iter()
            .rev()
            .find(|(r, h)| r.contains(pos) && self.hit_allowed(h))
            .map(|(r, h)| (*r, h.clone()))
    }

    /// Drawer listing for the current build (real on device, catalog on host).
    pub fn visible_drawer(&self) -> Vec<DrawerKey> {
        if platform::is_android() {
            self.drawer
                .iter()
                .filter(|entry| {
                    !self
                        .hidden_real
                        .iter()
                        .any(|hidden| hidden.package == entry.package)
                })
                .filter(|entry| letter_ok(&entry.label, self.search_letter))
                .map(|entry| DrawerKey::Real(entry.package.clone()))
                .collect()
        } else {
            icons::DRAWER
                .iter()
                .copied()
                .filter(|app| !self.hidden_apps.contains(app))
                .filter(|app| letter_ok(app.name(), self.search_letter))
                .map(DrawerKey::Mock)
                .collect()
        }
    }

    /// The full row for a real package, if it is installed.
    pub fn drawer_entry(&self, package: &str) -> Option<&DrawerEntry> {
        self.drawer.iter().find(|entry| entry.package == package)
    }

    /// Hidden Space rows in display order (mirrored by `views` and input).
    pub fn hidden_entries(&self) -> Vec<HiddenEntry> {
        merge_hidden(&self.hidden_apps, &self.hidden_real, self.hidden.items())
    }

    /// A ready icon texture for a package, if one has been built.
    pub fn icon_texture(&self, package: &str) -> Option<&egui::TextureHandle> {
        match self.icon_cache.get(package) {
            Some(IconSlot::Ready(texture)) => Some(texture),
            _ => None,
        }
    }

    /// The real component a reconstructed app resolved to (device).
    pub fn resolved(&self, app: App) -> Option<&RealApp> {
        self.resolved_targets.get(&app)
    }

    /// The installed package for a reconstructed app, if any.
    pub fn resolved_package(&self, app: App) -> Option<&str> {
        self.resolved(app).map(|real| real.package.as_str())
    }

    /// Whether quick-tile `i` is on (live on device, demo state on host).
    pub fn tile_on(&self, i: usize) -> bool {
        if platform::is_android() {
            match i {
                0 => self.system.wifi.unwrap_or(false),
                1 => self.system.bluetooth.unwrap_or(false),
                // The flashlight cannot be read or driven by an unprivileged
                // app, so the tile never claims to be on.
                2 => false,
                3 => self.system.airplane.unwrap_or(false),
                4 => self.system.auto_rotate.unwrap_or(false),
                5 => self.system.battery_saver.unwrap_or(false),
                _ => false,
            }
        } else {
            self.tiles.get(i).copied().unwrap_or(false)
        }
    }

    /// Shelf "Recent apps" rows (real launches on device).
    pub fn shelf_recents(&self) -> Vec<ShelfRecent> {
        if platform::is_android() {
            self.tasks
                .iter()
                .take(10)
                .map(|task| ShelfRecent {
                    mock: task.mock,
                    package: task.package.clone(),
                    activity: task.activity.clone(),
                    label: task.title.clone(),
                })
                .collect()
        } else {
            icons::SHELF_RECENTS
                .iter()
                .map(|app| ShelfRecent {
                    mock: Some(*app),
                    package: None,
                    activity: None,
                    label: app.name().to_string(),
                })
                .collect()
        }
    }

    /// Only hits belonging to the current surface are live.
    fn hit_allowed(&self, h: &Hit) -> bool {
        if self.open_app.is_some() {
            return matches!(h, Hit::AppBack | Hit::AppAction(_));
        }
        if self.onboard && self.screen == Screen::Home {
            return matches!(h, Hit::OnboardOk);
        }
        if self.locked_ui {
            return matches!(h, Hit::UnlockTap);
        }
        if self.screen == Screen::Shade {
            return matches!(
                h,
                Hit::ShadeTile(_)
                    | Hit::ShadeHandle
                    | Hit::ShadeClose
                    | Hit::ShadeBrightness
                    | Hit::ShadeClearAll
                    | Hit::ShadeNotif(_)
            );
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
            Screen::Drawer => matches!(
                h,
                Hit::DrawerApp(_)
                    | Hit::DrawerSearch
                    | Hit::DrawerSearchClear
                    | Hit::DrawerLetter(_)
                    | Hit::DrawerMenu
            ),
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
            Screen::Shade => false,
        }
    }

    fn shelf_open_or_visible(&self) -> bool {
        self.shelf_open || self.shelf_t > 0.02
    }

    fn open_drawer_hint(&mut self) {
        if !self.drawer_hint_shown {
            self.drawer_hint_shown = true;
            self.toast("Swipe right in the drawer for Hidden Space");
        }
    }

    // -------------------------------------------------------- device data

    /// Ask the loader for the installed-app list.
    fn request_apps(&mut self) {
        self.drawer_attempt_t = self.now;
        let outcome = match &self.loader {
            Some(loader) => Some(loader.request(Request::Apps)),
            None => None,
        };
        match outcome {
            Some(Ok(())) => {
                if !matches!(self.drawer_state, DrawerState::Ready) {
                    self.drawer_state = DrawerState::Loading;
                }
            }
            Some(Err(error)) => {
                let message = error.user_message();
                self.drawer_state = DrawerState::Failed(message.clone());
                self.toast(message);
            }
            None => {
                let message = PlatformError::NotInitialized.user_message();
                self.drawer_state = DrawerState::Failed(message.clone());
                self.toast(message);
            }
        }
    }

    /// Queue an icon request unless this package already has one.
    fn request_icon(&mut self, package: &str) {
        if self.icon_cache.contains_key(package) {
            return;
        }
        if platform::validate_package(package).is_err() {
            self.icon_cache
                .insert(package.to_owned(), IconSlot::Failed);
            return;
        }
        self.icon_cache.insert(package.to_owned(), IconSlot::Queued);
        let outcome = match &self.loader {
            Some(loader) => Some(loader.request(Request::Icon {
                package: package.to_owned(),
                size: ICON_TEXTURE_EDGE,
            })),
            None => None,
        };
        if let Some(Err(error)) = outcome {
            self.toast(error.user_message());
        }
    }

    /// Re-read the live toggle states (device only).
    fn refresh_states_if_device(&mut self) {
        if platform::is_android() {
            if let Ok(states) = platform::read_system_states() {
                self.system = states;
            }
        }
    }

    /// Launch a real component; on success the app moves to the recents top.
    fn launch_real(&mut self, real: &RealApp, origin: &str) {
        match platform::launch_app(real) {
            Ok(()) => {
                let mock = targets::app_for_package(&real.package);
                let accent = mock
                    .map(|app| app.accent())
                    .unwrap_or_else(|| targets::accent_for_package(&real.package));
                self.tasks.retain(|task| {
                    task.package.as_deref() != Some(real.package.as_str())
                });
                self.tasks.insert(
                    0,
                    Task {
                        mock,
                        package: Some(real.package.clone()),
                        activity: Some(real.activity.clone()),
                        title: real.label.clone(),
                        caption: origin.to_owned(),
                        accent,
                        resume_t: 1.0,
                        dismiss_t: 0.0,
                        dying: false,
                        x_off: 0.0,
                    },
                );
                self.tasks.truncate(8);
                self.carousel = 0;
            }
            Err(error) => self.toast(error.user_message()),
        }
    }

    /// Activate a reconstructed launcher entry.
    ///
    /// Device: resolve to a real component and launch it. Desktop: open the
    /// bundled demo preview.
    fn activate(&mut self, app: App, src: Rect, origin: &str) {
        if platform::is_android() {
            let real = self
                .resolved(app)
                .cloned()
                .or_else(|| targets::resolve(app, &[], false));
            match real {
                Some(real) => self.launch_real(&real, origin),
                None => self.toast(format!("{} isn't installed on this phone", app.name())),
            }
        } else {
            let name = app.name().to_string();
            self.tasks.insert(
                0,
                Task {
                    mock: Some(app),
                    package: None,
                    activity: None,
                    title: name.clone(),
                    caption: origin.to_owned(),
                    accent: app.accent(),
                    resume_t: 1.0,
                    dismiss_t: 0.0,
                    dying: false,
                    x_off: 0.0,
                },
            );
            self.tasks.truncate(6);
            self.carousel = 0;
            if self.screen == Screen::Shade {
                self.screen = Screen::Home;
            }
            self.open_app = Some(OpenApp { app, t: 0.0, src });
            self.toast(format!("{name} - demo preview"));
        }
    }

    /// Resume a recents card: bring it to the front and relaunch / preview it.
    fn resume(&mut self, i: usize, src: Rect) {
        if i >= self.tasks.len() {
            return;
        }
        let task = self.tasks.remove(i);
        self.carousel = 0;
        if platform::is_android() {
            let label = task.title.clone();
            let Some(package) = task.package.clone() else {
                self.tasks.insert(0, task);
                self.toast("This entry has no live app behind it");
                return;
            };
            let activity = task.activity.clone().or_else(|| {
                self.drawer
                    .iter()
                    .find(|entry| entry.package == package)
                    .map(|entry| entry.activity.clone())
            });
            self.tasks.insert(0, task);
            match activity {
                Some(activity) => {
                    let real = RealApp {
                        package,
                        activity,
                        label,
                    };
                    if let Err(error) = platform::launch_app(&real) {
                        self.toast(error.user_message());
                    }
                }
                None => self.toast("This entry can't be relaunched - open it from the drawer"),
            }
        } else {
            let label = task.title.clone();
            if let Some(mock) = task.mock {
                self.open_app = Some(OpenApp {
                    app: mock,
                    t: 0.0,
                    src,
                });
            }
            self.tasks.insert(0, task);
            self.toast(format!("Resumed {label} (demo)"));
        }
    }

    fn dismiss_task(&mut self, i: usize) {
        if let Some(t) = self.tasks.get_mut(i) {
            t.dying = true;
        }
    }

    fn long_press(
        &mut self,
        card: Option<String>,
        hide_key: Option<DrawerKey>,
        hidden_entry: Option<HiddenEntry>,
        menu: bool,
        bg: bool,
    ) {
        if let Some(tag) = card {
            if self.cards.delete_card("memo", &tag) == DeleteCardResult::Deleted {
                self.toast("Memo card deleted (deleteCard)");
            } else {
                self.toast("Card is publisher-owned (getCard only)");
            }
        } else if let Some(key) = hide_key {
            match key {
                DrawerKey::Mock(app) => {
                    if !self.hidden_apps.contains(&app) {
                        self.hidden_apps.push(app);
                    }
                    self.toast(format!("Hidden: {} - in Hidden Space", app.name()));
                }
                DrawerKey::Real(package) => {
                    if !self.hidden_real.iter().any(|h| h.package == package) {
                        let row = self
                            .drawer
                            .iter()
                            .find(|entry| entry.package == package)
                            .map(|entry| {
                                (
                                    entry.package.clone(),
                                    entry.activity.clone(),
                                    entry.label.clone(),
                                )
                            });
                        if let Some((package, activity, label)) = row {
                            self.request_icon(&package);
                            self.toast(format!("Hidden: {label} - in Hidden Space"));
                            self.hidden_real.push(HiddenReal {
                                package,
                                activity,
                                label,
                            });
                        }
                    }
                }
            }
        } else if let Some(entry) = hidden_entry {
            match entry {
                HiddenEntry::Mock(app) => {
                    self.hidden_apps.retain(|x| *x != app);
                    self.toast(format!("Restored {} to the drawer", app.name()));
                }
                HiddenEntry::Real(real) => {
                    self.hidden_real.retain(|x| x.package != real.package);
                    self.toast(format!("Restored {} to the drawer", real.label));
                }
                HiddenEntry::Demo(item) => {
                    self.hidden.unhide_item(&item.name);
                    self.toast(format!("Unhidden {} (demo)", item.name));
                }
            }
        } else if menu {
            let on = !self.hidden.password_enabled();
            self.hidden.set_password_enabled(on);
            if on {
                self.toast("Hidden Space password ON - hold the ring");
            } else {
                self.toast("Hidden Space password OFF");
            }
        } else if bg && self.screen == Screen::Home && !self.onboard && self.open_app.is_none() {
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
            hidden_entry: None,
            hide_key: None,
            menu: false,
            bg: hit.is_none(),
            unlock: false,
            status: false,
            bright: false,
            consumed: false,
        };
        match &hit {
            Some((_, Hit::ShelfCard(i))) => {
                if let Some(card) = self.cards.cards().get(*i) {
                    if card.channel == "memo" {
                        pr.card = Some(card.tag.clone());
                    }
                }
            }
            Some((_, Hit::HiddenItem(i))) => {
                let entries = self.hidden_entries();
                if let Some(entry) = entries.get(*i) {
                    pr.hidden_entry = Some(entry.clone());
                }
            }
            Some((_, Hit::DrawerApp(i))) => {
                let visible = self.visible_drawer();
                if let Some(key) = visible.get(*i) {
                    pr.hide_key = Some(key.clone());
                }
            }
            Some((_, Hit::HiddenMenu)) | Some((_, Hit::DrawerMenu)) => pr.menu = true,
            Some((_, Hit::HiddenHold)) => pr.unlock = true,
            _ => {}
        }
        pr.status = self.screen == Screen::Home
            && !self.locked_ui
            && self.open_app.is_none()
            && self.folder.is_none()
            && !self.shelf_open_or_visible()
            && pos.y < screen.top() + 26.0;
        pr.bright = self.screen == Screen::Shade
            && (pos.y - screen.top()) >= views::SHADE_BRIGHT_TOP
            && (pos.y - screen.top()) <= views::SHADE_BRIGHT_BOTTOM;
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

        if self.locked_ui || self.open_app.is_some() {
            return;
        }
        let (pr_unlock, pr_bright, pr_status, pr_x) = match &self.press {
            Some(pr) => (pr.unlock, pr.bright, pr.status, pr.pos.x),
            None => (false, false, false, 0.0),
        };
        if pr_unlock {
            return;
        }
        if pr_bright {
            // Brightness slider: map the finger onto 0.12..=1.0.
            let x0 = screen.left() + views::SHADE_SLIDER_LEFT;
            let x1 = screen.right() - views::SHADE_SLIDER_RIGHT;
            let f = ((pr_x + tdx - x0) / (x1 - x0).max(1.0)).clamp(0.0, 1.0);
            self.brightness = 0.12 + f * 0.88;
            return;
        }
        if pr_status {
            // Pull the shade down from the status bar.
            self.shade_t = (tdy / (h * 0.40)).clamp(0.0, 1.0);
            return;
        }
        if self.screen == Screen::Shade {
            // Push it back up.
            self.shade_t = (1.0 + tdy / (h * 0.40)).clamp(0.0, 1.0);
            return;
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
                let rows = self.visible_drawer().len().div_ceil(5);
                let content_h = rows as f32 * 96.0 + 40.0;
                let view_h = (h - 118.0).max(1.0);
                let max = (content_h - view_h).max(0.0);
                let want_close = tdy > 0.0 && self.drawer_scroll <= 0.5;
                if started_low && !want_close && tdy.abs() > tdx.abs() {
                    self.drawer_scroll = (self.drawer_scroll - d.y).clamp(0.0, max);
                } else if tdy > 0.0 && tdy.abs() > tdx.abs() {
                    self.drawer_t = (1.0 - tdy / (h * 0.45)).clamp(0.0, 1.0);
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
            Screen::Shade => {}
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
        let status = pr.as_ref().map(|p| p.status).unwrap_or(false);
        let from_left_edge = pr
            .as_ref()
            .map(|p| p.pos.x < screen.left() + 26.0)
            .unwrap_or(false);
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

        // App preview: chevron tap or left-edge swipe leaves it.
        if self.open_app.is_some() {
            if from_left_edge && dx > w * 0.15 {
                self.open_app = None;
                return;
            }
            if tap {
                self.handle_tap(pos, time);
            }
            return;
        }

        // Lock screen: swipe up (a tap also unlocks via handle_tap).
        if self.locked_ui {
            if -dy > h * 0.10 {
                self.locked_ui = false;
                self.toast("Unlocked (demo)");
            }
            return;
        }

        // Status-band pull-down opens the notification shade.
        if status {
            if self.shade_t > 0.35 || vel.y > 500.0 {
                self.screen = Screen::Shade;
                self.refresh_states_if_device();
            }
            return;
        }

        // Open shade: swipe up closes it.
        if self.screen == Screen::Shade {
            let out = state::classify_release(
                g.consumer,
                self.screen,
                dx,
                dy,
                vel.x,
                vel.y,
                w,
                h,
            );
            if matches!(out, ReleaseOutcome::GoHome) || self.shade_t < 0.6 {
                self.screen = Screen::Home;
            }
            return;
        }

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
            Screen::Shade => {}
        }
    }

    fn handle_tap(&mut self, pos: Pos2, time: f64) {
        let hit = self.hit_at(pos);
        match hit {
            Some((hr, Hit::Pill)) => {
                if platform::is_android() {
                    self.activate(App::Google, hr, "Home");
                } else {
                    self.toast("Google Search (demo)");
                }
            }
            Some((_, Hit::PageDot(i))) => {
                self.pager.current = i.min(self.pager.pages.saturating_sub(1));
                self.pager.offset = 0.0;
            }
            Some((_, Hit::Caret)) => {
                self.screen = Screen::Drawer;
                self.open_drawer_hint();
            }
            Some((hr, Hit::HomeItem { page, idx })) => match views::item_at(page, idx) {
                Some(views::HomeItem::App(a)) => self.activate(a, hr, "Home"),
                Some(views::HomeItem::Folder { .. }) => {
                    self.folder = Some((page, idx));
                }
                None => {}
            },
            Some((hr, Hit::Dock(i))) => {
                if let Some(app) = icons::DOCK.get(i) {
                    self.activate(*app, hr, "Home");
                }
            }
            Some((_, Hit::FolderClose)) => self.folder = None,
            Some((hr, Hit::FolderItem(i))) => {
                if let Some((page, idx)) = self.folder {
                    if let Some(views::HomeItem::Folder { apps, .. }) = views::item_at(page, idx) {
                        if let Some(app) = apps.get(i) {
                            self.activate(*app, hr, "Home");
                        }
                    }
                }
                self.folder = None;
            }
            Some((hr, Hit::DrawerApp(i))) => {
                let key = self.visible_drawer().get(i).cloned();
                match key {
                    Some(DrawerKey::Mock(app)) => self.activate(app, hr, "Apps"),
                    Some(DrawerKey::Real(package)) => {
                        let real = self.drawer_entry(&package).map(|entry| RealApp {
                            package: entry.package.clone(),
                            activity: entry.activity.clone(),
                            label: entry.label.clone(),
                        });
                        if let Some(real) = real {
                            self.launch_real(&real, "Apps");
                        }
                    }
                    None => {}
                }
                self.screen = Screen::Home;
            }
            Some((_, Hit::DrawerSearch)) => {
                if let DrawerState::Failed(_) = self.drawer_state {
                    if platform::is_android() {
                        self.toast("Retrying the app list\u{2026}");
                        self.request_apps();
                    }
                } else {
                    self.search_open = !self.search_open;
                }
            }
            Some((_, Hit::DrawerSearchClear)) => {
                self.search_letter = None;
                self.search_open = false;
            }
            Some((_, Hit::DrawerLetter(i))) => {
                if let Some(letter) = letter_at(i) {
                    self.search_letter = Some(letter);
                    self.search_open = false;
                    self.drawer_scroll = 0.0;
                }
            }
            Some((_, Hit::DrawerMenu)) => self.toast("Hold this glyph: Hidden Space password"),
            Some((_, Hit::HiddenClose)) => self.screen = Screen::Drawer,
            Some((_, Hit::HiddenAdd)) => {
                if platform::is_android() {
                    self.toast("Hold an app in the drawer to hide it here");
                } else {
                    self.toast("Add apps to Hidden Space (demo)");
                }
            }
            Some((_, Hit::HiddenMenu)) => self.toast("Hidden Space options"),
            Some((hr, Hit::HiddenItem(i))) => {
                let entry = self.hidden_entries().get(i).cloned();
                match entry {
                    Some(HiddenEntry::Mock(app)) => self.activate(app, hr, "Hidden Space"),
                    Some(HiddenEntry::Real(real)) => {
                        let component = RealApp {
                            package: real.package.clone(),
                            activity: real.activity.clone(),
                            label: real.label.clone(),
                        };
                        self.launch_real(&component, "Hidden Space");
                    }
                    Some(HiddenEntry::Demo(item)) => {
                        self.toast(format!("{} is preview content, not an app", item.name));
                    }
                    None => {}
                }
            }
            Some((_, Hit::HiddenHold)) => {}
            Some((hr, Hit::ShelfTool(i))) => {
                const NAMES: [&str; 5] = ["Calculator", "Selfie", "New timer", "Send files", "New tab"];
                if platform::is_android() {
                    // The toolbox rows are real apps on a device.
                    let target = match i {
                        0 => Some(App::Calculator),
                        1 => Some(App::Camera),
                        2 => Some(App::Clock),
                        3 => Some(App::Files),
                        4 => Some(App::Chrome),
                        _ => None,
                    };
                    if let Some(app) = target {
                        self.activate(app, hr, "Shelf");
                    }
                } else if let Some(name) = NAMES.get(i) {
                    self.toast(format!("Toolbox: {name} (demo)"));
                }
            }
            Some((hr, Hit::ShelfApp(i))) => {
                let recent = self.shelf_recents().get(i).cloned();
                match recent {
                    Some(row) => match (row.package, row.activity) {
                        (Some(package), Some(activity)) => {
                            let real = RealApp {
                                package,
                                activity,
                                label: row.label,
                            };
                            self.launch_real(&real, "Shelf");
                        }
                        _ => {
                            if let Some(mock) = row.mock {
                                self.activate(mock, hr, "Shelf");
                            }
                        }
                    },
                    None => {}
                }
            }
            Some((_, Hit::ShelfAction)) => {
                if platform::is_android() {
                    self.toast("Parking tracking needs location access this build doesn't request yet");
                } else {
                    self.toast("Parking Location: action (demo)");
                }
            }
            Some((_, Hit::ShelfNewNote)) => {
                self.memo_seq += 1;
                let tag = format!("memo-{}", self.memo_seq);
                let body = format!("Note #{}. Stored by the reconstructed CardProvider.", self.memo_seq);
                let _ = self.cards.post_card("memo", &tag, "Memo", &body, 0xFFB300);
                self.toast("postCard(memo)");
            }
            Some((_, Hit::ShelfCard(i))) => {
                if let Some(card) = self.cards.cards().get(i) {
                    let hyphen = if card.channel == "memo" { " - hold to delete" } else { "" };
                    self.toast(format!("getCard({}, {}){hyphen}", card.channel, card.tag));
                }
            }
            Some((hr, Hit::Task(i))) => {
                let src = Rect::from_center_size(hr.center(), Vec2::splat(96.0));
                self.resume(i, src);
            }
            Some((hr, Hit::TaskIcon(i))) => {
                let src = Rect::from_center_size(hr.center(), Vec2::splat(96.0));
                self.resume(i, src);
            }
            Some((_, Hit::TaskClearAll)) => {
                self.tasks.clear();
                self.carousel = 0;
                self.toast("Cleared all tasks");
            }
            Some((_, Hit::UnlockTap)) => {
                self.locked_ui = false;
                self.toast("Unlocked (demo)");
            }
            Some((_, Hit::AppBack)) => {
                self.open_app = None;
            }
            Some((_, Hit::AppAction(i))) => {
                let app = self.open_app.as_ref().map(|o| o.app);
                let msg = match (app, i) {
                    (Some(App::Camera), 0) => "Snap! (demo photo)".to_string(),
                    (Some(App::Camera), _) => "Camera flipped (demo)".to_string(),
                    (Some(App::PlayStore | App::PlayMovies), _) => "Install queued (demo)".to_string(),
                    (Some(App::Music | App::PlayMusic | App::Recorder), 1) => {
                        "Play / pause (demo)".to_string()
                    }
                    (Some(App::Weather), _) => "Forecast refreshed (demo)".to_string(),
                    (Some(a), _) => format!("{}: action {} (demo)", a.name(), i),
                    (None, _) => "Action (demo)".to_string(),
                };
                self.toast(msg);
            }
            Some((_, Hit::ShadeHandle)) | Some((_, Hit::ShadeClose)) => {
                self.screen = Screen::Home;
            }
            Some((_, Hit::ShadeTile(i))) => {
                if platform::is_android() {
                    const NAMES: [&str; 6] = [
                        "Wi-Fi",
                        "Bluetooth",
                        "Flashlight",
                        "Airplane mode",
                        "Display",
                        "Battery saver",
                    ];
                    if i == 2 {
                        self.toast("Flashlight control is reserved for system apps on Android 10+");
                        return;
                    }
                    let key = match i {
                        0 => "wifi",
                        1 => "bluetooth",
                        3 => "airplane",
                        4 => "display",
                        5 => "battery_saver",
                        _ => "",
                    };
                    match platform::open_system_screen(key) {
                        Ok(()) => {
                            self.screen = Screen::Home;
                            let name = NAMES.get(i).copied().unwrap_or("settings");
                            self.toast(format!("Opened {name} settings"));
                        }
                        Err(error) => self.toast(error.user_message()),
                    }
                } else {
                    const NAMES: [&str; 6] = [
                        "Wi-Fi",
                        "Bluetooth",
                        "Flashlight",
                        "Airplane",
                        "Auto-rotate",
                        "Battery saver",
                    ];
                    if let Some(t) = self.tiles.get_mut(i) {
                        *t = !*t;
                    }
                    if let Some(n) = NAMES.get(i) {
                        let on = self.tiles.get(i).copied().unwrap_or(false);
                        self.toast(format!("{n} {}", if on { "on" } else { "off" }));
                    }
                }
            }
            Some((_, Hit::ShadeBrightness)) => {}
            Some((_, Hit::ShadeClearAll)) => {
                self.notifs.clear();
                self.toast("Notifications cleared");
            }
            Some((hr, Hit::ShadeNotif(i))) => {
                let a = self.notifs.get(i).map(|n| n.app);
                if let Some(a) = a {
                    self.screen = Screen::Home;
                    self.activate(a, hr, "Home");
                }
            }
            Some((_, Hit::OnboardOk)) => {
                self.onboard = false;
            }
            None => {
                if self.screen == Screen::Home
                    && self.folder.is_none()
                    && !self.shelf_open
                    && !self.onboard
                    && self.open_app.is_none()
                {
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
        let shade_target = if self.screen == Screen::Shade { 1.0 } else { 0.0 };

        if self.active.is_none() {
            self.drawer_t = theme::approach(self.drawer_t, drawer_target, dt, 14.0);
            self.overview_t = theme::approach(self.overview_t, overview_target, dt, 14.0);
            self.lock_t = theme::approach(self.lock_t, lock_target, dt, 16.0);
            self.shade_t = theme::approach(self.shade_t, shade_target, dt, 14.0);
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

        // App preview reveal animation.
        if let Some(o) = &mut self.open_app {
            o.t = (o.t + dt * 3.4).min(1.0);
        }

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

        // Toast lifetime, then the next queued message.
        let mut clear = false;
        if let Some((_, ttl)) = self.toast.as_mut() {
            *ttl -= dt;
            clear = *ttl <= 0.0;
        }
        if clear {
            self.toast = None;
            if let Some(next) = self.toast_queue.pop_front() {
                self.toast = Some((next, 2.4));
            }
        }

        // Retry the installed-apps query while it keeps failing.
        if platform::is_android()
            && matches!(self.drawer_state, DrawerState::Failed(_))
            && self.now - self.drawer_attempt_t >= APPS_RETRY_SECONDS
        {
            self.request_apps();
        }

        // Long-press (0.65 s) on the latching targets.
        let fired = match &self.press {
            Some(pr) if !pr.consumed && self.now - pr.t > 0.65 => Some((
                pr.card.clone(),
                pr.hide_key.clone(),
                pr.hidden_entry.clone(),
                pr.menu,
                pr.bg,
            )),
            _ => None,
        };
        if let Some((card, hide_key, hidden_entry, menu, bg)) = fired {
            if let Some(p) = &mut self.press {
                p.consumed = true;
            }
            self.long_press(card, hide_key, hidden_entry, menu, bg);
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

    /// Convert finished loader work into UI state and textures.
    fn poll_loader(&mut self, ctx: &Context) {
        let replies = match &self.loader {
            Some(loader) => loader.drain(LOADER_REPLIES_PER_FRAME),
            None => return,
        };
        for reply in replies {
            match reply {
                Reply::Apps(Ok(apps)) => {
                    self.drawer = apps
                        .iter()
                        .map(|real| DrawerEntry {
                            package: real.package.clone(),
                            activity: real.activity.clone(),
                            label: real.label.clone(),
                            mock: targets::app_for_package(&real.package),
                        })
                        .collect();
                    self.resolved_targets = targets::resolve_all(&apps);
                    self.drawer_state = DrawerState::Ready;

                    // Queue icons for everything that can be drawn: the whole
                    // drawer, the resolved home/dock entries and hidden apps.
                    let mut packages: Vec<String> = self
                        .drawer
                        .iter()
                        .map(|entry| entry.package.clone())
                        .collect();
                    for real in self.resolved_targets.values() {
                        if !packages.iter().any(|package| package == &real.package) {
                            packages.push(real.package.clone());
                        }
                    }
                    for hidden in &self.hidden_real {
                        if !packages.iter().any(|package| package == &hidden.package) {
                            packages.push(hidden.package.clone());
                        }
                    }
                    let count = self.drawer.len();
                    for package in packages {
                        self.request_icon(&package);
                    }
                    if !self.apps_loaded_once {
                        self.apps_loaded_once = true;
                        self.toast(format!("{count} installed apps loaded"));
                    } else if self.apps_error_toasted {
                        self.toast("App list refreshed");
                    }
                    self.apps_error_toasted = false;
                }
                Reply::Apps(Err(error)) => {
                    if !self.apps_error_toasted {
                        self.apps_error_toasted = true;
                        self.toast(error.user_message());
                    }
                    log::warn!("apps query failed: {error}");
                    self.drawer_state = DrawerState::Failed(error.user_message());
                }
                Reply::Icon { package, result } => match result {
                    Ok(image) => {
                        let texture =
                            texture_from_image(ctx, format!("icon:{package}"), &image);
                        self.icon_cache
                            .insert(package, IconSlot::Ready(texture));
                    }
                    Err(error) => {
                        log::warn!("icon decode failed for {package}: {error}");
                        if !self.icon_error_shown {
                            self.icon_error_shown = true;
                            self.toast(error.user_message());
                        }
                        self.icon_cache.insert(package, IconSlot::Failed);
                    }
                },
                Reply::Wallpaper(Ok(image)) => {
                    self.wallpaper_tex =
                        Some(texture_from_image(ctx, "system-wallpaper".to_owned(), &image));
                }
                Reply::Wallpaper(Err(error)) => {
                    log::warn!("wallpaper load failed: {error}");
                    self.toast(error.user_message());
                }
            }
        }
    }

    /// Fetch the real wallpaper once the window size is known (device only).
    fn maybe_request_wallpaper(&mut self, ctx: &Context) {
        if !platform::is_android() || self.wallpaper_requested || self.loader.is_none() {
            return;
        }
        self.wallpaper_requested = true;
        let pixels_per_point = ctx.pixels_per_point();
        let size = ctx.screen_rect().size();
        let width = (size.x * pixels_per_point * 1.2).round() as u32;
        let height = (size.y * pixels_per_point).round() as u32;
        let outcome = match &self.loader {
            Some(loader) => Some(loader.request(Request::Wallpaper { width, height })),
            None => None,
        };
        if let Some(Err(error)) = outcome {
            self.toast(error.user_message());
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
        self.poll_loader(ctx);
        self.maybe_request_wallpaper(ctx);

        let painter = ctx.layer_painter(egui::LayerId::background());
        let rect = Rect::from_min_size(Pos2::ZERO, screen.size());
        views::paint(self, &painter, rect);

        ctx.request_repaint();
    }
}

/// Upload a decoded image as an egui texture (render thread only).
fn texture_from_image(ctx: &Context, name: String, image: &ImageRgba) -> egui::TextureHandle {
    let color = egui::ColorImage::from_rgba_unmultiplied(
        [image.width as usize, image.height as usize],
        &image.rgba,
    );
    ctx.load_texture(name, color, egui::TextureOptions::LINEAR)
}

/// Case-insensitive first-letter filter used by the drawer search field.
fn letter_ok(label: &str, letter: Option<char>) -> bool {
    match letter {
        None => true,
        Some(letter) => label
            .chars()
            .next()
            .map(|first| first.to_ascii_uppercase() == letter)
            .unwrap_or(false),
    }
}

/// Letter for a panel cell index (`0..26` -> `A..Z`).
fn letter_at(index: usize) -> Option<char> {
    if index < 26 {
        char::from_u32('A' as u32 + index as u32)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{letter_at, letter_ok};

    #[test]
    fn letter_filter_matches_case_insensitively() {
        assert!(letter_ok("Chrome", None));
        assert!(letter_ok("Chrome", Some('C')));
        assert!(letter_ok("chrome", Some('C')));
        assert!(!letter_ok("Chrome", Some('A')));
        assert!(!letter_ok("", Some('A')));
        assert!(letter_ok("1140", Some('1')));
    }

    #[test]
    fn letter_panel_indices_map_to_the_alphabet() {
        assert_eq!(letter_at(0), Some('A'));
        assert_eq!(letter_at(25), Some('Z'));
        assert_eq!(letter_at(26), None);
        assert_eq!(letter_at(999), None);
    }
}

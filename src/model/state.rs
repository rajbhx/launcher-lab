//! Gesture routing modelled on the input-consumer state machine observed in
//! `net.oneplus.quickstep.TouchInteractionService` (see `docs/ANALYSIS.md`,
//! section on the `*InputConsumer` family and its `TYPE_*` state space).

/// The kind of input consumer a gesture is routed to.
///
/// Names follow the classes found in the analyzed APK:
/// `NoOpInputConsumer`, `ResetGestureInputConsumer`, `OverviewInputConsumer`,
/// `OverviewWithoutFocusInputConsumer`, `DeviceLockedInputConsumer`,
/// `AssistantTouchConsumer`. `Shelf` and `Pager` are demo-level consumers
/// added for this reconstruction (the original handles those paths inside
/// the launcher activity rather than as standalone consumer classes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsumerType {
    NoOp,
    ResetGesture,
    Overview,
    OverviewWithoutFocus,
    DeviceLocked,
    Assistant,
    Shelf,
    Pager,
}

impl ConsumerType {
    /// Class name this consumer stands for.
    pub fn name(self) -> &'static str {
        match self {
            ConsumerType::NoOp => "NoOpInputConsumer",
            ConsumerType::ResetGesture => "ResetGestureInputConsumer",
            ConsumerType::Overview => "OverviewInputConsumer",
            ConsumerType::OverviewWithoutFocus => "OverviewWithoutFocusInputConsumer",
            ConsumerType::DeviceLocked => "DeviceLockedInputConsumer",
            ConsumerType::Assistant => "AssistantTouchConsumer",
            ConsumerType::Shelf => "ShelfEdgeConsumer (demo)",
            ConsumerType::Pager => "HomePagerConsumer (demo)",
        }
    }

    /// Short reason string shown in the in-app HUD.
    pub fn reason(self) -> &'static str {
        match self {
            ConsumerType::NoOp => "no-op",
            ConsumerType::ResetGesture => "reset-gesture",
            ConsumerType::Overview => "overview",
            ConsumerType::OverviewWithoutFocus => "overview-no-focus",
            ConsumerType::DeviceLocked => "device-locked",
            ConsumerType::Assistant => "assistant",
            ConsumerType::Shelf => "shelf-edge",
            ConsumerType::Pager => "home-pager",
        }
    }
}

/// Which top-level surface is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Home,
    /// App drawer (all-apps list) — the original `AllAppsContainerView`.
    Drawer,
    Overview,
    Hidden,
}

/// Edge zone a touch started in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GestureOrigin {
    BottomEdge,
    BottomLeftCorner,
    LeftEdge,
    TopEdge,
    Center,
}

/// Edge-zone sizes, as fractions of the screen.
#[derive(Clone, Copy, Debug)]
pub struct Zones {
    /// Bottom band that starts the overview / assistant gesture.
    pub bottom_edge: f32,
    /// Left band that starts the shelf edge-swipe.
    pub left_edge: f32,
    /// Top band used to distinguish reset-gesture touches in overview.
    pub top_edge: f32,
}

impl Default for Zones {
    fn default() -> Self {
        Self {
            bottom_edge: 0.16,
            left_edge: 0.08,
            top_edge: 0.12,
        }
    }
}

/// What a finished gesture should do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleaseOutcome {
    Stay,
    ShowOverview,
    GoHome,
    OpenShelf,
    NextPage,
    PrevPage,
    /// Swipe up in the middle of home: open the app drawer.
    OpenDrawer,
    /// Swipe right in the drawer: slide over to the Hidden Space.
    OpenHidden,
}

/// Decide which consumer a touch-down routes to, based on the current screen
/// and where the touch landed. `locked` mirrors the device-locked state that
/// `DeviceLockedInputConsumer` gates.
pub fn classify_start(
    screen: Screen,
    locked: bool,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    z: Zones,
) -> (ConsumerType, GestureOrigin) {
    if locked {
        return (ConsumerType::DeviceLocked, GestureOrigin::Center);
    }
    let rel_x = if w > 0.0 { x / w } else { 0.5 };
    let rel_y = if h > 0.0 { y / h } else { 0.5 };
    match screen {
        Screen::Home => {
            if rel_y >= 1.0 - z.bottom_edge {
                if rel_x <= 0.34 {
                    (ConsumerType::Assistant, GestureOrigin::BottomLeftCorner)
                } else {
                    (ConsumerType::Overview, GestureOrigin::BottomEdge)
                }
            } else if rel_x <= z.left_edge {
                (ConsumerType::Shelf, GestureOrigin::LeftEdge)
            } else {
                (ConsumerType::Pager, GestureOrigin::Center)
            }
        }
        Screen::Overview => {
            if rel_y <= z.top_edge {
                (ConsumerType::ResetGesture, GestureOrigin::TopEdge)
            } else {
                (ConsumerType::OverviewWithoutFocus, GestureOrigin::Center)
            }
        }
        Screen::Hidden => (ConsumerType::NoOp, GestureOrigin::Center),
        Screen::Drawer => (ConsumerType::NoOp, GestureOrigin::Center),
    }
}

/// Decide what a released gesture should do, given its displacement and
/// release velocity (px/frame-ish, from the pointer tracker).
pub fn classify_release(
    consumer: ConsumerType,
    screen: Screen,
    dx: f32,
    dy: f32,
    vx: f32,
    vy: f32,
    w: f32,
    h: f32,
) -> ReleaseOutcome {
    match (screen, consumer) {
        (Screen::Home, ConsumerType::Overview) => {
            if -dy > h * 0.10 || -vy > 550.0 {
                ReleaseOutcome::ShowOverview
            } else {
                ReleaseOutcome::Stay
            }
        }
        (Screen::Home, ConsumerType::Shelf) => {
            if dx > w * 0.22 || vx > 550.0 {
                ReleaseOutcome::OpenShelf
            } else {
                ReleaseOutcome::Stay
            }
        }
        (Screen::Home, ConsumerType::Pager) => {
            if -dy > h * 0.12 || -vy > 700.0 {
                ReleaseOutcome::OpenDrawer
            } else if dx > w * 0.25 || vx > 600.0 {
                ReleaseOutcome::PrevPage
            } else if -dx > w * 0.25 || -vx > 600.0 {
                ReleaseOutcome::NextPage
            } else {
                ReleaseOutcome::Stay
            }
        }
        (Screen::Drawer, ConsumerType::NoOp) => {
            if dx > w * 0.16 || vx > 500.0 {
                ReleaseOutcome::OpenHidden
            } else if dy > h * 0.10 || vy > 550.0 {
                ReleaseOutcome::GoHome
            } else {
                ReleaseOutcome::Stay
            }
        }
        (Screen::Overview, ConsumerType::ResetGesture)
        | (Screen::Overview, ConsumerType::OverviewWithoutFocus) => {
            if dy > h * 0.10 || vy > 550.0 {
                ReleaseOutcome::GoHome
            } else {
                ReleaseOutcome::Stay
            }
        }
        _ => ReleaseOutcome::Stay,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: f32 = 1000.0;
    const H: f32 = 2000.0;

    fn z() -> Zones {
        Zones::default()
    }

    #[test]
    fn locked_blocks_everything() {
        let (c, _) = classify_start(Screen::Home, true, 500.0, 1950.0, W, H, z());
        assert_eq!(c, ConsumerType::DeviceLocked);
        let (c2, _) = classify_start(Screen::Overview, true, 500.0, 100.0, W, H, z());
        assert_eq!(c2, ConsumerType::DeviceLocked);
    }

    #[test]
    fn bottom_edge_swipe_is_overview() {
        let (c, o) = classify_start(Screen::Home, false, 500.0, 1900.0, W, H, z());
        assert_eq!(c, ConsumerType::Overview);
        assert_eq!(o, GestureOrigin::BottomEdge);
    }

    #[test]
    fn bottom_left_corner_is_assistant() {
        let (c, o) = classify_start(Screen::Home, false, 200.0, 1950.0, W, H, z());
        assert_eq!(c, ConsumerType::Assistant);
        assert_eq!(o, GestureOrigin::BottomLeftCorner);
    }

    #[test]
    fn left_edge_is_shelf() {
        let (c, o) = classify_start(Screen::Home, false, 40.0, 1000.0, W, H, z());
        assert_eq!(c, ConsumerType::Shelf);
        assert_eq!(o, GestureOrigin::LeftEdge);
    }

    #[test]
    fn center_is_pager() {
        let (c, _) = classify_start(Screen::Home, false, 500.0, 1000.0, W, H, z());
        assert_eq!(c, ConsumerType::Pager);
    }

    #[test]
    fn overview_zones() {
        let (c, _) = classify_start(Screen::Overview, false, 500.0, 100.0, W, H, z());
        assert_eq!(c, ConsumerType::ResetGesture);
        let (c2, _) = classify_start(Screen::Overview, false, 500.0, 1000.0, W, H, z());
        assert_eq!(c2, ConsumerType::OverviewWithoutFocus);
    }

    #[test]
    fn hidden_is_noop() {
        let (c, _) = classify_start(Screen::Hidden, false, 500.0, 1000.0, W, H, z());
        assert_eq!(c, ConsumerType::NoOp);
    }

    #[test]
    fn release_up_from_bottom_shows_overview() {
        let out = classify_release(
            ConsumerType::Overview,
            Screen::Home,
            0.0,
            -300.0,
            0.0,
            -50.0,
            W,
            H,
        );
        assert_eq!(out, ReleaseOutcome::ShowOverview);
        let stay = classify_release(
            ConsumerType::Overview,
            Screen::Home,
            0.0,
            -40.0,
            0.0,
            -30.0,
            W,
            H,
        );
        assert_eq!(stay, ReleaseOutcome::Stay);
    }

    #[test]
    fn release_down_from_overview_goes_home() {
        let out = classify_release(
            ConsumerType::OverviewWithoutFocus,
            Screen::Overview,
            0.0,
            400.0,
            0.0,
            100.0,
            W,
            H,
        );
        assert_eq!(out, ReleaseOutcome::GoHome);
        let stay = classify_release(
            ConsumerType::OverviewWithoutFocus,
            Screen::Overview,
            0.0,
            50.0,
            0.0,
            20.0,
            W,
            H,
        );
        assert_eq!(stay, ReleaseOutcome::Stay);
    }

    #[test]
    fn shelf_release() {
        let open = classify_release(
            ConsumerType::Shelf,
            Screen::Home,
            300.0,
            0.0,
            100.0,
            0.0,
            W,
            H,
        );
        assert_eq!(open, ReleaseOutcome::OpenShelf);
        let stay = classify_release(
            ConsumerType::Shelf,
            Screen::Home,
            80.0,
            0.0,
            90.0,
            0.0,
            W,
            H,
        );
        assert_eq!(stay, ReleaseOutcome::Stay);
    }

    #[test]
    fn pager_release() {
        let next = classify_release(
            ConsumerType::Pager,
            Screen::Home,
            -400.0,
            0.0,
            -100.0,
            0.0,
            W,
            H,
        );
        assert_eq!(next, ReleaseOutcome::NextPage);
        let prev = classify_release(
            ConsumerType::Pager,
            Screen::Home,
            400.0,
            0.0,
            100.0,
            0.0,
            W,
            H,
        );
        assert_eq!(prev, ReleaseOutcome::PrevPage);
        let stay = classify_release(
            ConsumerType::Pager,
            Screen::Home,
            50.0,
            0.0,
            40.0,
            0.0,
            W,
            H,
        );
        assert_eq!(stay, ReleaseOutcome::Stay);
    }

    #[test]
    fn drawer_zone_is_noop() {
        let (c, o) = classify_start(Screen::Drawer, false, 500.0, 400.0, W, H, z());
        assert_eq!(c, ConsumerType::NoOp);
        assert_eq!(o, GestureOrigin::Center);
    }

    #[test]
    fn home_swipe_up_opens_drawer() {
        let out = classify_release(
            ConsumerType::Pager,
            Screen::Home,
            0.0,
            -250.0,
            0.0,
            -800.0,
            W,
            H,
        );
        assert_eq!(out, ReleaseOutcome::OpenDrawer);
    }

    #[test]
    fn drawer_swipe_right_opens_hidden_space() {
        let out = classify_release(
            ConsumerType::NoOp,
            Screen::Drawer,
            250.0,
            10.0,
            600.0,
            0.0,
            W,
            H,
        );
        assert_eq!(out, ReleaseOutcome::OpenHidden);
    }

    #[test]
    fn drawer_swipe_down_closes() {
        let out = classify_release(
            ConsumerType::NoOp,
            Screen::Drawer,
            0.0,
            300.0,
            0.0,
            700.0,
            W,
            H,
        );
        assert_eq!(out, ReleaseOutcome::GoHome);
    }
}

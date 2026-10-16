//! Curated launch targets for the reconstructed default workspace.
//!
//! The home pages, dock and the "Google"/"OnePlus" folders reproduce the
//! favourites of `res/xml/default_workspace_5x5_7.xml` from the original
//! OnePlus Launcher 4.4.8 package. The entries below map each reconstructed
//! [`App`] to the real Android components that implement it on OxygenOS 10,
//! in priority order:
//!
//! 1. packages and classes copied verbatim from the original XML
//!    (`className`/`packageName` attributes),
//! 2. package-only candidates for apps whose launcher class is not stable
//!    across OxygenOS builds - these resolve against the live
//!    `queryIntentActivities` list, which is authoritative.
//!
//! On a device every tap goes through [`resolve`] (live list first) and then
//! [`crate::platform::launch_app`]; the published mock painters only supply
//! the drawn fallback icon before the real `Bitmap` for a package arrives.

use crate::icons::App;
use crate::platform::RealApp;

/// One candidate Android component for a reconstructed launcher entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    /// Android package name (`packageName` in the workspace XML).
    pub package: &'static str,
    /// Fully-qualified launcher class when it is stable across builds.
    pub activity: Option<&'static str>,
}

const PHONE: &[Target] = &[
    Target {
        package: "com.oneplus.dialer",
        activity: Some("com.android.dialer.oneplus.activity.OPDialtactsActivity"),
    },
    Target {
        package: "com.android.dialer",
        activity: Some("com.oneplus.contacts.activities.OPDialtactsActivity"),
    },
    Target {
        package: "com.google.android.dialer",
        activity: Some("com.google.android.dialer.extensions.GoogleDialtactsActivity"),
    },
];

const MESSAGES: &[Target] = &[
    Target {
        package: "com.oneplus.mms",
        activity: Some("com.android.mms.ui.ConversationList"),
    },
    Target {
        package: "com.google.android.apps.messaging",
        activity: None,
    },
];

const CHROME: &[Target] = &[Target {
    package: "com.android.chrome",
    activity: Some("com.google.android.apps.chrome.Main"),
}];

const CAMERA: &[Target] = &[Target {
    package: "com.oneplus.camera",
    activity: Some("com.oneplus.camera.OPCameraActivity"),
}];

const WEATHER: &[Target] = &[Target {
    package: "net.oneplus.weather",
    activity: Some("net.oneplus.weather.app.MainActivity"),
}];

const GALLERY: &[Target] = &[Target {
    package: "com.oneplus.gallery",
    activity: Some("com.oneplus.gallery.OPGalleryActivity"),
}];

const PLAY_STORE: &[Target] = &[Target {
    package: "com.android.vending",
    activity: Some("com.android.vending.AssetBrowserActivity"),
}];

const GOOGLE: &[Target] = &[Target {
    package: "com.google.android.googlequicksearchbox",
    activity: Some("com.google.android.googlequicksearchbox.SearchActivity"),
}];

const GMAIL: &[Target] = &[Target {
    package: "com.google.android.gm",
    activity: Some("com.google.android.gm.ConversationListActivityGmail"),
}];

const MAPS: &[Target] = &[Target {
    package: "com.google.android.apps.maps",
    activity: Some("com.google.android.maps.MapsActivity"),
}];

const YOUTUBE: &[Target] = &[Target {
    package: "com.google.android.youtube",
    activity: Some("com.google.android.youtube.app.honeycomb.Shell$HomeActivity"),
}];

const DRIVE: &[Target] = &[Target {
    package: "com.google.android.apps.docs",
    activity: Some("com.google.android.apps.docs.app.NewMainProxyActivity"),
}];

const PLAY_MUSIC: &[Target] = &[Target {
    package: "com.google.android.music",
    activity: Some("com.android.music.activitymanagement.TopLevelActivity"),
}];

const PLAY_MOVIES: &[Target] = &[Target {
    package: "com.google.android.videos",
    activity: Some("com.google.android.youtube.videos.EntryPoint"),
}];

const GOOGLE_PAY: &[Target] = &[Target {
    package: "com.google.android.apps.walletnfcrel",
    activity: Some("com.google.commerce.tapandpay.android.cardlist.CardListActivity"),
}];

const DUO: &[Target] = &[Target {
    package: "com.google.android.apps.tachyon",
    activity: Some("com.google.android.apps.tachyon.MainActivity"),
}];

const PHOTOS: &[Target] = &[Target {
    package: "com.google.android.apps.photos",
    activity: Some("com.google.android.apps.photos.home.HomeActivity"),
}];

const NETFLIX: &[Target] = &[Target {
    package: "com.netflix.mediaclient",
    activity: Some("com.netflix.mediaclient.ui.launch.UIWebViewActivity"),
}];

const SETTINGS: &[Target] = &[Target {
    package: "com.android.settings",
    activity: Some("com.android.settings.Settings"),
}];

const FILES: &[Target] = &[Target {
    package: "com.oneplus.filemanager",
    activity: Some("com.oneplus.filemanager.HomePageActivity"),
}];

const MUSIC: &[Target] = &[
    Target {
        package: "com.oneplus.music",
        activity: None,
    },
    Target {
        package: "com.google.android.music",
        activity: Some("com.android.music.activitymanagement.TopLevelActivity"),
    },
    Target {
        package: "com.spotify.music",
        activity: None,
    },
];

const NOTES: &[Target] = &[
    Target {
        package: "com.oneplus.note",
        activity: None,
    },
    Target {
        package: "com.microsoft.office.onenote",
        activity: None,
    },
    Target {
        package: "com.google.android.keep",
        activity: None,
    },
];

const CLOCK: &[Target] = &[
    Target {
        package: "com.oneplus.deskclock",
        activity: None,
    },
    Target {
        package: "com.google.android.deskclock",
        activity: Some("com.android.deskclock.DeskClock"),
    },
    Target {
        package: "com.android.deskclock",
        activity: Some("com.android.deskclock.DeskClock"),
    },
];

const CALCULATOR: &[Target] = &[
    Target {
        package: "com.oneplus.calculator",
        activity: None,
    },
    Target {
        package: "com.google.android.calculator",
        activity: Some("com.android.calculator2.Calculator"),
    },
];

const RECORDER: &[Target] = &[Target {
    package: "com.oneplus.recorder",
    activity: None,
}];

const FORUMS: &[Target] = &[Target {
    package: "net.oneplus.forums",
    activity: Some("net.oneplus.forums.ui.activity.CheckPermissionActivity"),
}];

const BACKUP: &[Target] = &[Target {
    package: "com.oneplus.backuprestore",
    activity: Some("com.oneplus.backuprestore.activity.BootActivity"),
}];

/// Candidate components for a reconstructed app, in priority order.
pub fn targets(app: App) -> &'static [Target] {
    match app {
        App::Phone => PHONE,
        App::Messages => MESSAGES,
        App::Chrome => CHROME,
        App::Camera => CAMERA,
        App::Weather => WEATHER,
        App::Gallery => GALLERY,
        App::PlayStore => PLAY_STORE,
        App::Google => GOOGLE,
        App::Gmail => GMAIL,
        App::Maps => MAPS,
        App::YouTube => YOUTUBE,
        App::Drive => DRIVE,
        App::PlayMusic => PLAY_MUSIC,
        App::PlayMovies => PLAY_MOVIES,
        App::GooglePay => GOOGLE_PAY,
        App::Duo => DUO,
        App::Photos => PHOTOS,
        App::Netflix => NETFLIX,
        App::Settings => SETTINGS,
        App::Files => FILES,
        App::Music => MUSIC,
        App::Notes => NOTES,
        App::Clock => CLOCK,
        App::Calculator => CALCULATOR,
        App::Recorder => RECORDER,
        App::Forums => FORUMS,
        App::Backup => BACKUP,
    }
}

/// Resolve a reconstructed app to a real component.
///
/// While the live app list exists (`list_ready`), only packages that the
/// device actually reported count - a stale static class must never launch
/// something the query did not see. Before the first list arrives, candidates
/// with a stable class are usable so the very first tap after startup still
/// works.
pub fn resolve(app: App, installed: &[RealApp], list_ready: bool) -> Option<RealApp> {
    let candidates = targets(app);
    for target in candidates {
        if let Some(found) = installed.iter().find(|real| real.package == target.package) {
            return Some(found.clone());
        }
    }
    if list_ready {
        return None;
    }
    for target in candidates {
        if let Some(activity) = target.activity {
            return Some(RealApp {
                package: target.package.to_owned(),
                activity: activity.to_owned(),
                label: app.name().to_owned(),
            });
        }
    }
    None
}

/// Resolve every reconstructed app (host keeps its projections separate).
pub fn resolve_all(installed: &[RealApp]) -> std::collections::HashMap<App, RealApp> {
    let mut map = std::collections::HashMap::new();
    for app in crate::icons::DRAWER {
        if let Some(real) = resolve(app, installed, true) {
            map.insert(app, real);
        }
    }
    map
}

/// The reconstructed painter that corresponds to an installed package, if any.
pub fn app_for_package(package: &str) -> Option<App> {
    crate::icons::DRAWER
        .iter()
        .copied()
        .find(|app| targets(*app).iter().any(|target| target.package == package))
}

/// A stable accent colour for a package without a curated painter.
pub fn accent_for_package(package: &str) -> u32 {
    const ACCENTS: [u32; 8] = [
        0x2A82F0, 0x16B879, 0xE8452C, 0x8A63F4, 0x1F9C6C, 0xC77D2B, 0x2BB6A3, 0x60707C,
    ];
    let mut hash: u64 = 0xCBF2_9CE4_8422_2325;
    for byte in package.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01B3);
    }
    ACCENTS[(hash % ACCENTS.len() as u64) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{validate_component, validate_package};

    fn real(package: &str, activity: &str, label: &str) -> RealApp {
        RealApp {
            package: package.to_owned(),
            activity: activity.to_owned(),
            label: label.to_owned(),
        }
    }

    #[test]
    fn every_drawer_app_has_targets() {
        for app in crate::icons::DRAWER {
            assert!(!targets(app).is_empty(), "{app:?} has no targets");
        }
    }

    #[test]
    fn target_components_validate() {
        for app in crate::icons::DRAWER {
            for target in targets(app) {
                assert!(
                    validate_package(target.package).is_ok(),
                    "{app:?} package {} is invalid",
                    target.package
                );
                if let Some(activity) = target.activity {
                    assert!(
                        validate_component(target.package, activity).is_ok(),
                        "{app:?} component {} / {} is invalid",
                        target.package,
                        activity
                    );
                }
            }
        }
    }

    #[test]
    fn resolve_prefers_installed_entries() {
        let installed = vec![real(
            "com.oneplus.dialer",
            "com.android.dialer.oneplus.activity.OPDialtactsActivity",
            "Phone",
        )];
        let resolved = resolve(App::Phone, &installed, true).expect("phone resolved");
        assert_eq!(resolved.package, "com.oneplus.dialer");
        assert_eq!(
            resolved.activity,
            "com.android.dialer.oneplus.activity.OPDialtactsActivity"
        );
        assert_eq!(resolved.label, "Phone");
    }

    #[test]
    fn resolve_honours_priority_order() {
        let installed = vec![
            real(
                "com.google.android.dialer",
                "com.google.android.dialer.extensions.GoogleDialtactsActivity",
                "Phone",
            ),
            real(
                "com.android.dialer",
                "com.oneplus.contacts.activities.OPDialtactsActivity",
                "Phone",
            ),
        ];
        let resolved = resolve(App::Phone, &installed, true).expect("phone resolved");
        assert_eq!(resolved.package, "com.android.dialer");
    }

    #[test]
    fn resolve_requires_a_live_entry_when_ready() {
        assert!(resolve(App::Netflix, &[], true).is_none());
    }

    #[test]
    fn resolve_falls_back_while_loading() {
        let resolved = resolve(App::Settings, &[], false).expect("settings resolved");
        assert_eq!(resolved.activity, "com.android.settings.Settings");
        // Fuzzy candidates with no stable class cannot resolve before the list.
        assert!(resolve(App::Recorder, &[], false).is_none());
    }

    #[test]
    fn package_lookup_round_trips() {
        assert_eq!(app_for_package("com.android.vending"), Some(App::PlayStore));
        assert_eq!(app_for_package("com.oneplus.camera"), Some(App::Camera));
        assert_eq!(app_for_package("dev.example.unknown"), None);
    }

    #[test]
    fn resolve_all_keeps_only_installed_apps() {
        assert!(resolve_all(&[]).is_empty());
        let installed = vec![
            real("com.android.chrome", "com.google.android.apps.chrome.Main", "Chrome"),
            real("dev.unknown.game", "dev.unknown.game.Main", "Game"),
        ];
        let map = resolve_all(&installed);
        assert_eq!(map.get(&App::Chrome).map(|r| r.package.as_str()), Some("com.android.chrome"));
        assert!(!map.contains_key(&App::Netflix));
    }

    #[test]
    fn accents_are_stable_and_bounded() {
        let accents: Vec<u32> = ["com.a.one", "com.b.two", "com.a.one"]
            .iter()
            .map(|package| accent_for_package(package))
            .collect();
        assert_eq!(accents[0], accents[2]);
        for accent in accents {
            assert!(accent <= 0x00FF_FFFF, "accent {accent:#08x} out of range");
        }
    }
}

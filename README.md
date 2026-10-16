# Launcher Lab

A **gesture-first Android launcher prototype written in pure Rust** — no Java, no Kotlin, no
garbage collector, no AndroidX. Just a `cdylib` that `cargo-apk` packs into an installable APK.

Launcher Lab reconstructs the interaction model that a static reverse-engineering pass
(REA + JADX, see [`docs/ANALYSIS.md`](docs/ANALYSIS.md)) recovered from **OnePlus Launcher
4.4.8** (`net.oneplus.launcher`, versionCode 20408, minSdk/targetSdk 29): the
`*InputConsumer` gesture-routing state machine, the Shelf card/channel contract, the Hidden
Space flag model, and the recents/Overview flow.

> **Status:** v0.4.0 — real device integration. On Android every surface is backed by the
> phone itself: the drawer lists the real installed apps, taps launch the real apps, icons and
> the wallpaper stream in from the system, and the shade reads live toggle states (a normal
> app cannot flip radios on Android 10+, so tiles open the real settings screens). The desktop
> build keeps the bundled demo catalog. The APK is signed with a project key and built
> entirely by GitHub Actions.

## Download

Grab the APK from the rolling release:

**<https://github.com/rajbhx/launcher-lab/releases/tag/apk-latest>**

The file is `launcher-lab.apk`, rebuilt automatically by GitHub Actions on every push to
`main`. Every build also appears under the workflow's *Artifacts*.

### Install on a OnePlus 7

1. Download `launcher-lab.apk` on the phone (or via USB/any file transfer).
2. Open it from **Files → Downloads**.
3. Allow *"install unknown apps"* for whatever app opens it when prompted.
4. Install, then open **Launcher Lab** from the app drawer.
   It is a normal app — it does **not** replace your system home screen.

Requirements: arm64-v8a device, Android 10+ (minSdk 29). Built on GitHub's Ubuntu runners
against the Android NDK (r26d); nothing is compiled on a local machine.

## What it does

The home screen reconstructs the stock OxygenOS 10 look from the decoded resources
(`default_workspace_5x5_7.xml`, `dimens.xml`, `device_profiles.xml`): icons, the
clock/weather widget with the signature red leading digit, the Google pill, the dock, and the
`^` caret + page dots are all drawn live with `egui` primitives — no image assets ship with
the app.

On a device, tapping an app (home, dock, drawer, folder, shelf or a recents card) launches the
real installed app — the drawer is your phone's actual app list, and the home wallpaper is the
real system wallpaper with parallax. Icons are read from the system at runtime; the drawn set
in `src/icons.rs` remains the fallback while a texture loads. The desktop dev build (no
Android) keeps the in-app **demo previews**: tapping an icon opens a full-screen mock with a
zoom-reveal animation — Chrome with an address bar and result rows, Messages with chat bubbles,
Camera with a viewfinder and shutter, Gallery with a photo grid, Play Store with Install rows,
Music with a player, plus Weather, Clock, Calculator, Maps and video surfaces; everything else
gets a skeleton page. Back via the chevron or a left-edge swipe. A first-run card maps every
gesture.

| Gesture / UI element | Behaviour (reconstructed from) |
| --- | --- |
| Swipe left / right on home | 5-page pager (`default_workspace_5x5_7.xml` page layout) |
| Swipe right from the **left edge**, or right-drag on page 1 | **Shelf** slides in from the left |
| Swipe **up** in the middle of home | App **drawer** (all-apps grid, alphabetical) |
| Swipe **up from the bottom edge** | **Recents** (Overview) — `OverviewInputConsumer` |
| Swipe **down** in recents / drawer | Back home — `ResetGestureInputConsumer` |
| Tap the **^** caret above the page dots | Open the drawer; dots jump pages |
| Bottom-left corner touch | Assistant zone (`AssistantTouchConsumer`) |
| Drawer: swipe **right** | **Hidden Space**; swipe down closes; drag scrolls the grid |
| Hidden Space: hold the ring | Unlock; hold the ⋮ menu glyph to toggle the password flag (`HiddenSpaceModel`) |
| Hidden items: hold an icon | Restore it to the drawer; tap = open (real app on device, preview on desktop) |
| Drawer: hold an app | Hide it (a real package on device); hold it in Hidden Space to restore it |
| Swipe **down from the status bar** | **Notification shade**: clock, quick tiles with live states, brightness, notifications |
| Shade: tap a tile / drag the slider | Tiles open the real settings screens (device) or toggle demo tiles (desktop); the slider dims the launcher surface |
| Tap an app | Launches the real app (device) or the demo preview (desktop) |
| Recents: swipe up on a card | Dismiss the task; drag across = carousel; Clear all empties it. Cards are the apps you really launched |
| Shelf: tap a card | `getCard` readout; hold a memo → `deleteCard`; "+ New note" → `postCard` |
| Hold empty home | Light/dark chrome toggle (dark pill ↔ light pill) |
| Double-tap empty home | Device-locked overlay (`DeviceLockedInputConsumer`); swipe up or tap to unlock |

The shelf's Toolbox launches the matching real apps on device (Calculator, Camera, Clock,
Files, Chrome). Recent apps lists what you actually launched, newest first, and re-taps
relaunch. Parking Location is an honest reconstruction on device (live tracking would need
location access this build does not request); the desktop preview keeps the full painted map.
Below them, cards come from the reconstructed `CardProvider` channel store: the `memo`
channel works on every platform ("+ New note" / hold a memo to delete), while the
`weather` / `health` / `music` publishers post in the desktop preview.

Unit tests cover the pure model: gesture classification & release outcomes
(`src/model/state.rs`), pager physics (`src/model/pager.rs`), the card contract
(`src/model/shelf.rs`), the hidden-space model including real hidden packages
(`src/model/hidden.rs`), the UTC civil-date clock math (`src/clock.rs`), the
app→component target map (`src/targets.rs`), the loader protocol (`src/loader.rs`),
and platform value validation (`src/platform.rs`).

## Architecture

```
src/
├── lib.rs        crate root + desktop entry
├── main.rs       desktop binary (`cargo run`)
├── android.rs    Android entry (`android_main`, NativeActivity)
├── app.rs        state, input routing, gesture lifecycle, device data pipeline
├── views.rs      painter-only rendering of every surface + hit zones
├── icons.rs      app-icon painters (fallback for packages without a texture)
├── platform.rs   the JNI backend: installed apps, launching, icons, wallpaper, system state
├── loader.rs     single worker thread between platform (JNI) and the UI
├── targets.rs    reconstructed home/dock entries → real installed components
├── wallpaper.rs  procedural wallpaper (desktop + fallback while the real one loads)
├── clock.rs      wall-clock time (`localtime_r` on Android, UTC civil-date on host)
├── theme.rs      palette + bundled Roboto families + animation helpers
└── model/        pure logic, unit-tested
    ├── state.rs  input-consumer routing (the reconstructed state machine)
    ├── pager.rs  home-screen pager
    ├── shelf.rs  CardContract-style store
    └── hidden.rs Hidden Space model (demo items + real hidden packages)
assets/
└── fonts/        Roboto Regular/Light/Medium (Apache-2.0, see LICENSE.txt)
```

Rendering is `egui`/`eframe` on the **glow (OpenGL ES) backend**: the Android entry point
explicitly requests `Renderer::Glow` with `vsync: true`, so frames are drawn by the GPU into
the `NativeActivity`'s hardware-accelerated EGL surface and paced at the display refresh
rate. The UI is 100% custom-painted: rounded tiles, task cards, shelf, unlock ring,
notification shade, wallpaper — no widgets, no bitmaps. Frame-rate independent exponential
animations keep transitions smooth.

## How it's built

### Locally (Windows host)

`scripts/build-apk.ps1` runs the same pipeline on a local machine: it locates the Android
SDK (build-tools + platform) and NDK, points `aarch64-linux-android` at the NDK clang
wrapper, runs `cargo apk build --release --lib`, then `zipalign -p 4` and `apksigner sign`
with the project keystore (read from `-KeystoreDir`, default `C:\Users\raj\launcher-lab-keys`,
never written into the repo), and finally re-verifies the signature and prints the SHA-256.

```powershell
pwsh -File scripts\build-apk.ps1              # signed, aligned APK
pwsh -File scripts\build-apk.ps1 -SkipSign    # unsigned, for inspection
```

Prerequisites: Rust stable + `cargo install cargo-apk`, a JDK on `JAVA_HOME`, Android
build-tools + platform 34 and an NDK (verified with NDK 26.1 / build-tools 34.0.0 on Java 21).

### On CI (GitHub Actions)

Two workflows, zero local builds required:

* **`.github/workflows/ci.yml`** — `cargo check --all-targets` + `cargo test` on
  `ubuntu-24.04` for every push: model tests and host compilation.
* **`.github/workflows/android.yml`** — the APK pipeline:
  1. JDK 17, Android SDK (platform 34, build-tools 33.0.2), NDK **r26d**
  2. Rust stable + `aarch64-linux-android` target, `cargo install cargo-apk`
  3. `cargo apk build --release --lib`
  4. verifies the APK with `aapt2 dump badging` + `apksigner verify`
  5. uploads it as a workflow artifact and refreshes the `apk-latest` release

`Cargo.toml` carries the Android packaging metadata
(`package.metadata.android.*`): application id `dev.rajbhx.launcherlab`, minSdk 29,
targetSdk 34, `build_targets = ["aarch64-linux-android"]`, portrait, fullscreen theme.

## Limitations

Some things are impossible for an unprivileged app on Android 10+, and the build stays honest
about them instead of faking them:

* **Notifications** need a `NotificationListenerService`, which only the system picker can
  grant; on a device the shade's notification area stays visibly empty with an explanation
  (the desktop build shows demo notifications).
* **Recent-app snapshots** need privileged APIs: the Overview shows your real launches (icon,
  title, caption) with an explicit "no live snapshot" note instead of a counterfeit screenshot.
* **Quick-setting toggles** cannot flip Wi-Fi/Bluetooth/airplane from a normal app; tiles read
  the live state and open the corresponding system settings screen. The flashlight tile is
  system-reserved and never claims to be on.
* The **brightness slider** dims the launcher's own surface, not the system brightness
  (that needs `WRITE_SETTINGS`, reserved for system apps).
* The built APK requests `ACCESS_WIFI_STATE` and `BLUETOOTH` — used solely to read the live
  tile states. Nothing else is collected, and no other permission is requested.
* Signed with a project-generated key (stable across builds, so in-place updates work), but
  it is not a Play-ready signing setup.
* Only `arm64-v8a` is built — fine for the OnePlus 7, not for old 32-bit devices.
* The launcher does not register as a HOME replacement on purpose (safe to try).

## License

MIT — see [LICENSE](LICENSE). Not affiliated with OnePlus; "OnePlus Launcher" is referenced
only to describe what was analyzed.

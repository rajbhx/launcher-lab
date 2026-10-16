# Launcher Lab

A **gesture-first Android launcher prototype written in pure Rust** — no Java, no Kotlin, no
garbage collector, no AndroidX. Just a `cdylib` that `cargo-apk` packs into an installable APK.

Launcher Lab reconstructs the interaction model that a static reverse-engineering pass
(REA + JADX, see [`docs/ANALYSIS.md`](docs/ANALYSIS.md)) recovered from **OnePlus Launcher
4.4.8** (`net.oneplus.launcher`, versionCode 20408, minSdk/targetSdk 29): the
`*InputConsumer` gesture-routing state machine, the Shelf card/channel contract, the Hidden
Space flag model, and the recents/Overview flow.

> **Status:** demo (v0.3.0). It is a real, installable launcher-shaped app — every icon opens
> a full-screen reconstructed preview of that app (so the demo reads everywhere), the recents
> list is simulated, and the APK is signed with a project key (built and signed entirely by
> GitHub Actions).

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

Every app gets an in-app **demo preview**: tapping an icon (home, dock, drawer, folder, shelf,
recents or a notification) opens a full-screen mock with a zoom-reveal animation — Chrome
with an address bar and result rows, Messages with chat bubbles, Camera with a viewfinder and
shutter, Gallery with a photo grid, Play Store with Install rows, Music with a player, plus
Weather, Clock, Calculator, Maps and video surfaces; everything else gets a skeleton page.
Back via the chevron or a left-edge swipe. A first-run card maps every gesture.

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
| Hidden items: hold an icon | Unhide (`unhideItem`); tap = open demo |
| Drawer: hold an app | Hide it (moves to Hidden Space); hold it there to restore it |
| Swipe **down from the status bar** | **Notification shade**: clock, quick tiles, brightness, notifications |
| Shade: tap a tile / drag the slider | Quick-setting toggles; the brightness slider dims the whole surface |
| Tap an app or a notification | In-app demo preview; back = chevron tap or left-edge swipe |
| Recents: swipe up on a card | Dismiss the task; drag across = carousel; Clear all empties it |
| Shelf: tap a card | `getCard` readout; hold a memo → `deleteCard`; "+ New note" → `postCard` |
| Hold empty home | Light/dark chrome toggle (dark pill ↔ light pill) |
| Double-tap empty home | Device-locked overlay (`DeviceLockedInputConsumer`); swipe up or tap to unlock |

The shelf's Toolbox / Recent apps / Parking Location cards mirror the stock 4.4.8 shelf,
followed by live cards from the reconstructed `CardProvider` channel store
(`weather.home`, `health.steps`, `music.now`, `memo.*`).

Unit tests cover the pure model: gesture classification & release outcomes
(`src/model/state.rs`), pager physics (`src/model/pager.rs`), the card contract
(`src/model/shelf.rs`), the hidden-space flag model (`src/model/hidden.rs`), and the
UTC civil-date clock math (`src/clock.rs`).

## Architecture

```
src/
├── lib.rs        crate root + desktop entry
├── main.rs       desktop binary (`cargo run`)
├── android.rs    Android entry (`android_main`, NativeActivity)
├── app.rs        state, input routing, gesture lifecycle
├── views.rs      painter-only rendering of every surface + hit zones
├── icons.rs      every app icon, composed from primitives
├── wallpaper.rs  procedural wallpaper (painterly clouds over indigo)
├── clock.rs      wall-clock time (`localtime_r` on Android, UTC civil-date on host)
├── theme.rs      palette + bundled Roboto families + animation helpers
└── model/        pure logic, unit-tested
    ├── state.rs  input-consumer routing (the reconstructed state machine)
    ├── pager.rs  home-screen pager
    ├── shelf.rs  CardContract-style store
    └── hidden.rs Hidden Space model
assets/
└── fonts/        Roboto Regular/Light/Medium (Apache-2.0, see LICENSE.txt)
```

Rendering is `egui`/`eframe` on the **glow (OpenGL ES) backend**: the Android entry point
explicitly requests `Renderer::Glow` with `vsync: true`, so frames are drawn by the GPU into
the `NativeActivity`'s hardware-accelerated EGL surface and paced at the display refresh
rate. The UI is 100% custom-painted: rounded tiles, task cards, shelf, unlock ring,
notification shade, wallpaper — no widgets, no bitmaps. Frame-rate independent exponential
animations keep transitions smooth.

## How it's built (GitHub Actions)

Two workflows, zero local builds:

* **`.github/workflows/ci.yml`** — `cargo check --all-targets` + `cargo test` on
  `ubuntu-latest` for every push: model tests and host compilation.
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

* Signed with a project-generated key (stable across builds, so in-place updates work), but
  it is not a Play-ready signing setup.
* Tiles, tasks and cards are demo data; launching opens reconstructed in-app previews, not
  real apps (that would need the system's privileged recents/task APIs).
* Only `arm64-v8a` is built — fine for the OnePlus 7, not for old 32-bit devices.
* The launcher does not register as a HOME replacement on purpose (safe to try).

## License

MIT — see [LICENSE](LICENSE). Not affiliated with OnePlus; "OnePlus Launcher" is referenced
only to describe what was analyzed.

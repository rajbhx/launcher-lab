# Launcher Lab

A **gesture-first Android launcher prototype written in pure Rust** — no Java, no Kotlin, no
garbage collector, no AndroidX. Just a `cdylib` that `cargo-apk` packs into an installable APK.

Launcher Lab reconstructs the interaction model that a static reverse-engineering pass
(REA + JADX, see [`docs/ANALYSIS.md`](docs/ANALYSIS.md)) recovered from **OnePlus Launcher
4.4.8** (`net.oneplus.launcher`, versionCode 20408, minSdk/targetSdk 29): the
`*InputConsumer` gesture-routing state machine, the Shelf card/channel contract, the Hidden
Space flag model, and the recents/Overview flow.

> **Status:** demo. It is a real, installable launcher-shaped app — but tiles don't launch
> other apps, the recents list is simulated, and the APK is debug-signed.

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

Everything in the app is one of the reconstructed subsystems, made tangible:

| Gesture / UI element | Reconstructed subsystem (see `docs/ANALYSIS.md`) |
| --- | --- |
| Swipe up from the bottom edge → **Recents/Overview** rises; swipe down → back home | `OverviewInputConsumer` / `ResetGestureInputConsumer`, the `TYPE_OVERVIEW` / `TYPE_RESET_GESTURE` state space of `net.oneplus.quickstep.TouchInteractionService` |
| Start a touch in the **bottom-left corner** → assistant bubble | `AssistantTouchConsumer` |
| **LOCK** chip → all gestures stop routing, overlay says so; tap to unlock | `DeviceLockedInputConsumer` |
| Swipe from the **left edge** → **Shelf** panel slides over the home grid | edge-swipe shelf surface |
| Shelf shows **channel cards** (`weather.home`, `health.steps`, `music.now`, `memo.*`) | `CardProvider` / `CardContract` binder API: register channel, `postCard(channel, tag)` |
| **+ MEMO** posts a card; posting the same `(channel, tag)` updates it | `PostCardResult::Posted` / `Updated` semantics |
| Tap a memo → `getCard` readout; **hold** a memo → `deleteCard` | `getCard` / `deleteCard` by `(channel, tag)` |
| **Pinch out** on home → **Hidden Space**; pinch in to leave | Hidden Space entry gesture |
| Hidden Space **password** toggle + hold-to-unlock pad | `HiddenSpaceModel` password flag gating access |
| Drag a recents card sideways / tap to resume | recents task lifecycle in the overview |
| Live HUD pill at the top while you drag | shows which consumer class the gesture routed to |

Unit tests cover the pure model: gesture classification & release outcomes
(`src/model/state.rs`), pager physics (`src/model/pager.rs`), the card contract
(`src/model/shelf.rs`), and the hidden-space flag model (`src/model/hidden.rs`).

## Architecture

```
src/
├── lib.rs        crate root + desktop entry
├── main.rs       desktop binary (`cargo run`)
├── android.rs    Android entry (`android_main`, NativeActivity)
├── app.rs        state, input routing, gesture lifecycle
├── views.rs      painter-only rendering (every control hand-drawn) + hit zones
├── theme.rs      palette + animation helpers
└── model/        pure logic, unit-tested
    ├── state.rs  input-consumer routing (the reconstructed state machine)
    ├── pager.rs  home-screen pager
    ├── shelf.rs  CardContract-style store
    └── hidden.rs Hidden Space model
```

Rendering is `egui`/`eframe` (GLES via glow). The UI is 100% custom-painted: rounded tiles,
task cards, shelf, unlock ring — no widgets, no assets. Frame-rate independent exponential
animations keep transitions smooth.

## How it's built (GitHub Actions)

Two workflows, zero local builds:

* **`.github/workflows/ci.yml`** — `cargo check --all-targets` + `cargo test` on
  `ubuntu-latest` for every push: model tests and host compilation.
* **`.github/workflows/android.yml`** — the APK pipeline:
  1. JDK 17, Android SDK (platform 34, build-tools 33.0.2), NDK **r26d**
  2. Rust stable + `aarch64-linux-android` target, `cargo install cargo-apk`
  3. `cargo apk build --release`
  4. verifies the APK with `aapt2 dump badging` + `apksigner verify`
  5. uploads it as a workflow artifact and refreshes the `apk-latest` release

`Cargo.toml` carries the Android packaging metadata
(`package.metadata.android.*`): application id `dev.rajbhx.launcherlab`, minSdk 29,
targetSdk 34, `build_targets = ["aarch64-linux-android"]`, portrait, fullscreen theme.

## Limitations

* Debug-signed, so Android shows the usual sideloading prompts and it can't be updated from Play.
* Tiles, tasks and cards are demo data; nothing launches real apps (that would need the
  system's privileged recents/task APIs).
* Only `arm64-v8a` is built — fine for the OnePlus 7, not for old 32-bit devices.
* The launcher does not register as a HOME replacement on purpose (safe to try).

## License

MIT — see [LICENSE](LICENSE). Not affiliated with OnePlus; "OnePlus Launcher" is referenced
only to describe what was analyzed.

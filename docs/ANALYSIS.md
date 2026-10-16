# OnePlus Launcher 4.4.8 — REA Static Analysis Dossier

- **Artifact:** `OnePlusLauncher-4.4.8.200529184345.a93df77.apk` (15,127,151 bytes)
- **SHA-256:** `30cd82e306fc1e79c974a3eb2e4bf3e33151c97370965b79af144c184a1f05c5`
- **Tooling:** REA Android provider — JADX headless engine (`jadx-headless-mcp 0.7.1`, bridge `40c6972b…`, heap 992 MiB, worker count 1)
- **Method:** static analysis only; the APK was never executed
- **Evidence bundle:** `rea-evidence-bundle.json` (5,762,715 bytes, 51 evidence records) — contains the full decoded manifest (34,975 chars) and the complete 6,402-class inventory

---

## 1. Package identity

| Field | Value |
|---|---|
| Package | `net.oneplus.launcher` |
| Version | 4.4.8.200529184345.a93df77 (versionCode 20408) |
| minSdk / targetSdk | 29 / 29 (Android 10) |
| Classes | 6,402 |
| Resources | 2,428 |
| Permissions | 56 (see §3) |
| Signature verification | not performed by this provider |

## 2. Provider environment notes (for reproducible reruns)

Two transient failures occurred on first use; both were diagnosed as cold-start latency, not defects:

| Stage | Deadline | First cold run | Warm run |
|---|---|---|---|
| Workspace guardian (`WindowsPrivateRuntime.create`) | 15 s | ~18.1 s (fail) | ~2.4 s (ok) |
| JADX bridge MCP `initialize` handshake | 30 s | >30 s (fail) | ~13.1 s (ok) |

Cause: freshly copied 40 MB JAR + Windows Defender first-touch scanning. Everything works once caches are warm. JVM args used by REA: `-XX:ActiveProcessorCount=2`, default heap.

## 3. Manifest census

- 22 activities · 15 services · 15 receivers · 16 providers
- Full decoded XML: inside the evidence bundle (`manifest.text`)

**Component highlights (verified names):** `net.oneplus.launcher.Launcher`, `net.oneplus.quickstep.RecentsActivity`, `net.oneplus.quickstep.LockScreenRecentsActivity`, `net.oneplus.quickstep.TouchInteractionService`, `net.oneplus.shelf.card.GeneralCardProvider`, `net.oneplus.launcher.quickpage.ExternalWidgetProvider`, `net.oneplus.launcher.hiddenspace.HiddenSpaceProvider`.

**Permissions (56)** — grouped by AOSP protection class (grouping = analyst inference, declaration = evidence):

- **Privileged / system-level observed:** `CONTROL_REMOTE_APP_TRANSITION_ANIMATIONS`, `READ_FRAME_BUFFER`, `START_ANY_ACTIVITY`, `DEVICE_POWER`, `EXPAND_STATUS_BAR`, `BIND_APPWIDGET`, `KILL_BACKGROUND_PROCESSES`, `READ_PRIVILEGED_PHONE_STATE`, `REQUEST_DELETE_PACKAGES`, `WRITE_SECURE_SETTINGS`, `STOP_APP_SWITCHES`, `GET_APP_OPS_STATS`, `FORCE_STOP_PACKAGES`, `INJECT_EVENTS`, `INTERACT_ACROSS_USERS`, `INTERACT_ACROSS_USERS_FULL`, `MANAGE_ACTIVITY_STACKS`, `MANAGE_APP_OPS_MODES`, `MANAGE_USERS`, `STATUS_BAR_SERVICE`, `STATUS_BAR`, `UPDATE_APP_OPS_STATS`, `USE_RESERVED_DISK`
  - Quirk observed: the literal `android.Manifest.permission.WATCH_APPOPS` is declared with a mixed-case package prefix (will not resolve to the platform constant `WATCH_APPOPS`); observation only.
- **Launcher/vendor custom:** `com.android.launcher.permission.READ_SETTINGS`, `com.android.launcher.permission.WRITE_SETTINGS`, `net.oneplus.launcher.permission.EXTERNAL_WIDGET`, `net.oneplus.shelf.permission.CARD`, `com.verizon.permission.CHANGE_BADGE`
- **Standard/user-facing:** `ACCESS_NETWORK_STATE`, `CHANGE_NETWORK_STATE`, `CALL_PHONE`, `READ_CALENDAR`, `WRITE_CALENDAR`, `READ_CONTACTS`, `READ_EXTERNAL_STORAGE`, `WRITE_EXTERNAL_STORAGE`, `RECEIVE_BOOT_COMPLETED`, `SET_WALLPAPER`, `SET_WALLPAPER_COMPONENT`, `SET_WALLPAPER_HINTS`, `VIBRATE`, `FOREGROUND_SERVICE`, `WRITE_SETTINGS`, `ACCESS_INSTANT_APPS`, `ACCESS_COARSE_LOCATION`, `ACCESS_FINE_LOCATION`, `CAMERA`, `INTERNET`, `ACCESS_WIFI_STATE`, `READ_PHONE_STATE`, `BLUETOOTH`, `ACTIVITY_RECOGNITION`, `WAKE_LOCK`
- **SDK/messaging:** `com.google.android.c2dm.permission.RECEIVE`, `com.google.android.finsky.permission.BIND_GET_INSTALL_REFERRER_SERVICE`

*Inference (qualified):* the privileged set is consistent with a system-signed platform launcher on OxygenOS; declaration ≠ grant ≠ execution.

## 4. Subsystem map

| Subsystem | Namespace | Classes |
|---|---|---|
| Gesture nav / Recents engine | `net.oneplus.quickstep` (+ `inputconsumers`, `legacygesture`, `fallback`) | 100 |
| Shelf card framework | `net.oneplus.shelf` (`card`, `card.result`, `card.typeadapter`) | 34 in `card` alone |
| Hidden Space | `net.oneplus.launcher.allapps` + `hiddenspace` + extras | 19 names |
| Gesture tutorial / gesture UI | various | 18 matches on `Gesture` |

### 4a. Shelf card framework (`net.oneplus.shelf.card.*` — full inventory)

`ApiKeyHelper`, `BlankStyle`, `Card`, `CardContract`, `CardDatabaseHelper`, `CardHelper`, `CardImageStorageHelper`, `CardManager`, `CardModel`, `CardProvider`, `Channel`, `ChannelManager`, `CustomStyle`, `GeneralCard`, `GeneralCardProvider`, `Image`, `ImageContentStyle`, `ImageUploadHelper`, `ListViewStyle`, `MemoMigrationUtil`, `NotePanelStyle`, `TextContentStyle` + `result.*` (PostCard, GetCard, DeleteCard, GetChannel, RegisterChannel, IsCardPosted, IsChannelRegistered, ComposeJson) + `typeadapter.*` (ImageStyle, Intent, Operation).

## 5. Deep dives

### 5a. The input-consumer state machine (`net.oneplus.quickstep.inputconsumers`)

12 consumers: `InputConsumer` (base), `AccessibilityInputConsumer`, `AssistantTouchConsumer`, `DelegateInputConsumer`, `DeviceLockedInputConsumer`, `FallbackHomeInputConsumer`, `LandscapeInputConsumer`, `OtherActivityInputConsumer`, `OverviewInputConsumer`, `OverviewWithoutFocusInputConsumer`, `ResetGestureInputConsumer`, `ScreenPinnedInputConsumer`.

Base contract methods: `onInputEvent` / `onMotionEvent` / `onKeyEvent`, `getType` / `getName` / `getReason`, `allowInterceptByParent()`, `onConsumerAboutToBeSwitched()`, `useSharedSwipeState()`.

`TYPE_*` constants (state space): `TYPE_NO_OP`, `TYPE_OVERVIEW`, `TYPE_OVERVIEW_WITHOUT_FOCUS`, `TYPE_OTHER_ACTIVITY`, `TYPE_LEGACY_OVERVIEW`, `TYPE_LEGACY_OTHER_ACTIVITY`, `TYPE_DEVICE_LOCKED`, `TYPE_SCREEN_PINNED`, `TYPE_FALLBACK_HOME`, `TYPE_FALLBACK_NO_BUTTON`, `TYPE_RESET_GESTURE`, `TYPE_ASSISTANT`, `TYPE_ACCESSIBILITY`.

`TouchInteractionService` (57 methods / 42 fields) creates the consumers (`createOverviewInputConsumer`, `createOtherActivityInputConsumer`, `createDeviceLockedInputConsumer`) and gates gestures (`disableHorizontalSwipe`, `canTriggerAssistantAction`, `inGestureTutorialMode`, `getNavbarSize`). Static incoming references: 16 using nodes, coverage complete — e.g. `net.oneplus.launcher.uioverrides.RecentsUiFactory`, `uioverrides.states.OverviewState`.

Decompiled sample (`disableHorizontalSwipe`, Java, no smali fallback):

```java
private boolean disableHorizontalSwipe(MotionEvent motionEvent) {
    Region region = this.mExclusionRegion;
    boolean z = this.mMode == SysUINavigationMode.Mode.NO_BUTTON && region != null
            && region.contains((int) motionEvent.getX(), (int) motionEvent.getY());
    return !this.mMode.hasNavBar
            ? z | this.mSwipeTouchRegion.contains((int) motionEvent.getX(), (int) motionEvent.getY())
            : z;
}
```

*Inference:* exclusion/touch regions gate horizontal swipes depending on navigation mode (no-button vs navbar) — Android 10 gestural-navigation region handling.

### 5b. Shelf card binder API

`CardProvider` (5 methods / 1 field): `isAtShelf()`, `onDeleted(Context,String,int)`, `onReceive(Context,Intent)`, `onUpdate(Context,CardManager,String,int)`; field `atShelf`.

`CardContract` (31 fields / 3 methods) — the cross-process protocol: authority `CONTENT_AUTHORITY`, base URI, methods `METHOD_POST_CARD_BY_CHANNEL_ID_AND_TAG`, `METHOD_DELETE_CARD_BY_CHANNEL_ID_AND_TAG`, `METHOD_GET_CARD_BY_CHANNEL_ID(_AND_TAG)`, `METHOD_CLEAR_CARD_BY_CHANNEL_ID`, `METHOD_CHECK_CARD_POSTED`, `METHOD_CHECK_SHELF_VERSION`; keys `KEY_CARD_DATA`, `KEY_CARD_ID`, `KEY_CARD_POSTED`, `KEY_CHANNEL_ID`, `KEY_CHANNEL_TOKEN`, `KEY_CALLER_PACKAGE`, `KEY_RESULT_CODE`, `KEY_SHELF_ENABLED`, `KEY_TAG`, `KEY_VERSION`; paths `PATH_CARD`, `PATH_CARD_IMAGE`, `PATH_CHANNEL`, `PATH_CHANNEL_IMAGE`; `POST` / `DELETE` verbs.

*Inference:* a content-provider-based pub/sub channel model: third-party apps register a channel, then post/update/delete cards identified by `(channelId, tag)`; result classes map to structured outcomes.

### 5c. Hidden Space

19 classes including `HiddenSpaceProvider`, `HiddenSpaceModel`, `HiddenSpacePresenter`, `HiddenSpaceContract`, `HiddenSpaceFloatingView(+Presenter/RecyclerView)`, `HiddenSpaceTipsView`, `HiddenTutorialPageIndicatorDots`, `HiddenSpacePinchOutStateChangeTouchController`, `AlphabeticalHiddenAppsList`, `HiddenAppsItemAnimator`, plus data-layer `AppDBHelperHidden`, `AppInfoHidden`, `AppProviderHidden`, `HiddenAppsRemoveFromWorkspaceModelWriter`.

`HiddenSpaceModel` itself is tiny (3 methods): `isHiddenSpacePasswordEnabled` / `setHiddenSpacePasswordEnabled` — the heavier logic lives in the presenter/provider pair.

## 6. Evidence index (full IDs)

| Evidence | ID |
|---|---|
| Package summary | `ev_4125d8e4e8d2354a36f9f3cb6b5f1af732653fa89c395a9bf700c30930a3ffcb` |
| Package complete (manifest text) | `ev_d6d3c1be37ee018d946c4391f61b7237e7e7df01bff9806a4b65c95a6f4fea40` |
| Permission summary re-fetch | `ev_3268aaf1671d545671411fbb03ddf8235924b2a5899e856be1edb31c404430ee` |
| Class search: quickstep | `ev_a87120c1c2f64c87ef98c0ad22c9b7e034d473e2c45bd167a860b27de74894c8` |
| Full 6,402-class inventory | `ev_4e0af33d59763aaac576a0896471c9a9349e05d89fbf9cf8cf75f1b159b3421b` |
| Class: TouchInteractionService | `ev_9349ba537837da5a2ec8d3b0e2bed78ea90b17307a1e0e5445a26ab8e70e567f` |
| Method: disableHorizontalSwipe | `ev_b5be88620601efcab60e8dc95d6ad818ab0aceef27701aa2b478bf4df9371dd3` |
| Class search: quickstep inventory | `ev_9048960022431352bc597ec26397ce6eafce7657ea4ca6096aa07c2d350fb651` |
| Class search: shelf | `ev_71baff628d5bda8786476217da80c20de4162f775cae04bf4c54321fda582c15` |
| Class search: Gesture | `ev_cfc36f730b20b6590902b644c36ba8bd16831a3b6b810adcbf98e066fe2753ec` |
| Class search: hiddenspace | `ev_ea4136cf1f56596d8ac6f65326fc596b4adbe833ea6f409a958428a8f75872f8` |
| Class search: Hidden (broad) | `ev_4f7fcf201e1486be60de9b53f4c6e9351370965e97c9d04f6fd7c20ebf197118` |
| Class search: shelf.card | `ev_3edb424e98ae13c074cc418c6d5ba866360405ed1f9a504ae657ee181b72b48f` |
| Class: CardProvider | `ev_4092660358f99fff6d6480559d3279a4cd98126bea706b5a0a31f41fce3e39c2` |
| Class: CardContract | `ev_c06aa1e91533e0bff1e1d00e33757b9df376cc52d774a4e5f67fd08fab1a9468` |
| Class search: inputconsumers | `ev_e8ae82d4e10b3c7cc8d7043eddb603ef78c63885bf2079111678bbd15cffd15f` |
| Class: InputConsumer (base) | `ev_5c938e71a6d65c5562ad58238112f07ef719844c977b3cb9168726579dd405ce` |
| Class: HiddenSpaceModel | `ev_52fbba398567638f7bd6a5f3ebe87621f35c0e8283e66eb00467d0a920571165` |

## 7. Second-pass verification (local toolchain, apktool 2.9.3)

A second static pass was run on the same artifact (SHA-256 unchanged,
`30cd82e306fc1e79c974a3eb2e4bf3e33151c97370965b79af144c184a1f05c5`) two ways: the local
`apktool` 2.9.3 (`d --no-src`, 2,713 files decoded, `coverage: complete`) for the manifest
census and the build-relevant resources, and a fresh REA pass (JADX provider +
`rea-artifact-graph`) for code and inventory claims. The interactive REA session was briefly
lost to a host restart and re-established; §1–§6 stand as recorded.

| Second-pass Evidence | ID |
|---|---|
| Resource decode (apktool 2.9.3, manifest text) | `ev_a8c90c52131a3cebbe8646a127082fd2cc5e674f3bab9b052c54bd7e7e87220a` |
| Package re-fetch (JADX) | `ev_3ddd2bfdf6f9561fd7d310bcc8271972353435fe33641ed8a35a55fda4356bcd` |
| ZIP inventory | `ev_1a0b2d93b15d65e3c6f0c38fdd44e596047e40ac4dcb6893d0294657a6f3b655` |
| `inspect_artifact` (complete) | `ev_073e8e0708872f037de02d62881fc2418d7986e7bcde91d858cf9eda2ba3e26c` |
| Android application projection (`adp_862e7307…`) | `ev_57a7a33c85f73fc87f71612e1d9af67367a50320326da7e43c4abeb0f83ad917` |
| Class: `net.oneplus.launcher.Launcher` (246 methods) | `ev_7573d4be237516a03bc955a6ce061d6a93ad550574affe408f2fbf563eade1ff` |
| Class: `net.oneplus.launcher.LauncherApplication` (11 methods) | `ev_e53fbcc052e3ff688ccb8e1efaeee132e5928f481c755df7f646722cf3773553` |
| Method: `Launcher.onCreate` | `ev_cfdf4c653a9a11c20cae007d45adf7ebd72e42c175ad8416f3b11a00eb6fb034` |
| Class search: `Workspace` (16) | `ev_ad54d8e579bf3bd594aac9c8a58e6201e792e66da2dbd021aab136a4b8f62578` |
| Class search: `net.oneplus.launcher.uioverrides` (28) | `ev_790b789234ba6d5a7652aea0e4409ccdf1f0e767b7ff80bd356922a3a7d1c84d` |

**Census (exact, decoded manifest, 30,137 bytes):** 22 `<activity>` · 15 `<service>` ·
15 `<receiver>` · 16 `<provider>` · 7 `<permission>` declarations · 56 `<uses-permission>` ·
16 `<meta-data>` — independently reproducing the JADX-side census in §3.

**Code / loader shape (REA inventory projection, `inventory_complete: true`):** the APK holds
exactly 1 `AndroidManifest.xml`, 1 `resources.arsc`, 2 DEX files (`classes.dex`,
`classes2.dex`), **0 native libraries**, 0 JavaScript bundles, and 3 v1 signing members
(`META-INF/CERT.RSA|CERT.SF|MANIFEST.MF`). Runtime family: `dalvik-art` only; bridge
candidates: none. (ZIP inventory sees central-directory entries only, so it cannot resolve
the APK Signing Block — the v2/v3 signature status is not established by this projection.)
**The APK ships no native code at all**: the launcher is DEX plus resources, and every
privileged surface it leans on is provided by the OxygenOS platform rather than a bundled
`.so`. Kotlin `*.kotlin_module` metadata lives under `kotlin/`.

**Launcher core observed (second pass).** `net.oneplus.launcher.Launcher` carries 246
methods and the appwidget/request constants (`APPWIDGET_HOST_ID`, `REQUEST_BIND_APPWIDGET`,
`REQUEST_ENTER_HIDDEN_SPACE`, `REQUEST_CREATE_QUICK_PANEL_WIDGET`, …) plus the whole
`bind*` model-update surface (`bindAllApplications`, `bindAppsAdded`, `bindWidgetsRestored`,
`bindDeepShortcutMap`, `bindMinusOneScreenContent`). `LauncherApplication` is thin: 11
methods, with `initGoogleAnalytics` / `initCrashlytics` analytics plumbing and a `DRY_RUN`
flag; `onCreate` calls `setTheme(Themes.isDarkTheme(this) ? MainThemeDark : MainTheme)` and
refuses to run — killing its own process — when the cached device width disagrees with the
current display (`LauncherAppState` width conflict) or the current user is invalid.

The companion state/gesture drivers to §5a live in `net.oneplus.launcher.uioverrides` (28
classes): states `AllAppsState`, `BackgroundAppState`, `OverviewPeekState`, `OverviewState`,
`QuickSwitchState`, and touch controllers `NavBarToHomeTouchController`,
`FlingAndHoldTouchController`, `QuickSwitchTouchController`, `OverviewToAllAppsTouchController`,
`TaskViewTouchController`, `StatusBarTouchController`, `PortraitStatesTouchController`,
`LandscapeEdgeSwipeController`, `RecentsOperationController`. The `Workspace` surface itself
is 16 classes — `Workspace`, `WorkspaceItemInfo`, `WorkspaceLayoutManager`,
`WorkspaceStateTransitionAnimation`, `pageindicators.{WorkspacePageIndicator,OnePlusWorkspacePageIndicator,OnePlusWorkspacePageIndicatorTips}`,
`graphics.WorkspaceAndHotseatScrim`, `model.{Add,Remove}WorkspaceItemsTask`,
`touch.{WorkspaceTouchListener,WorkspaceOptionModeTouchHelper}`.

**Home-screen layout evidence used by the reconstruction:**

| Fact | Source (decoded resource) |
|---|---|
| Page 0 widgets: Google QSB 5×1 at (0,0); weather (3×1) and clock (3×1) both declared at (0,1) — regional variants | `res/xml/default_workspace_5x5_7.xml` |
| Hotseat (`container=-101`): dialer (screen 0), messages (1), chrome (2), camera (3), gallery (4) | same |
| Two folders (`google_folder_title`, `oneplus_folder_title`) on pages 0/1; Play Store pinned at (4,4) | same |
| Icon size 56 dp, 14.4 sp labels, 5×5 at ≥694×406 dp ("Large Phone") | `res/xml/device_profiles.xml` |
| `page_indicator_bottom_margin` 9 dp (6 dp fullscreen), `overview_mode_items_width` 84.57 dp | `res/values-sw400dp-xxhdpi/dimens.xml` (OnePlus-7-class width) |
| Accent palette `accent_charm_purple` / `fashion_pink` / `golden` / `grass_green` / `lemon_yellow` / `sky_blue` / `vigour_red`, `accent_star_wars` `#ffff2837`, `launcher_bg_color` `#884e8391` | `res/values/colors.xml` |
| `LauncherTheme`: transparent status + navigation bars, `windowDrawsSystemBarBackgrounds` | `res/values/styles.xml` |

**Vendor metadata observed:** application class `net.oneplus.launcher.LauncherApplication`,
`LauncherBackupAgent`, `@style/LauncherTheme` (transparent status and navigation bars),
required `uses-feature com.oneplus.software.oos`, `oneplus_libs`
`oplib.oneplus_sdk_wrapper:0.1.0;oplib.oneplus_sdk_utils:0.2.2`, `upgrade_product_code=40013`,
Fabric/Crashlytics + Firebase + GMS analytics entries, and a Google Maps v2 API key
(`com.google.android.maps.v2.API_KEY`) shipped in plaintext in the manifest.

Per-device overrides live in `values-sw400dp-xxhdpi`, `values-sw370dp-xxhdpi`,
`values-sw360dp-xxhdpi`, `values-sw360dp`, `values-sw340dp`; `values-2873x1440` is the
1440p (7T Pro-class) variant. 117 locale folders are present, matching the `locales` list
(§3).

## 8. Reproduced artifact (Launcher Lab, pure Rust)

This repository is the reconstruction: a Rust `cdylib` that `cargo-apk` packages into
`dev.rajbhx.launcherlab`, exposing exactly one activity — `android.app.NativeActivity`, with
`android:hasCode="false"` and `android.app.lib_name=launcher_lab`. That is the structural
inverse of the analyzed package (DEX + resources, no native code): **no DEX at all**, and the
only native code is the launcher itself.

Built, aligned and signed locally with build-tools 34.0.0 / NDK 26.1.10909125 / JDK 21:

| Field | Value |
|---|---|
| Output | `target/release/apk/launcher-lab.apk` |
| Size | 2,962,238 bytes |
| SHA-256 | `c091e2314d7b2b7f98d2bb8a5cf0641e18e802e472a31964c7c448e7804182a9` |
| Entries | `AndroidManifest.xml`, `resources.arsc`, 5× `res/mipmap-*/ic_launcher.png`, `lib/arm64-v8a/liblauncher_lab.so` |
| Native entry | AArch64 ELF exporting `ANativeActivity_onCreate` and `android_main` |
| Signature | verified, `CN=Launcher Lab, OU=Demo, O=rajbhx, C=IN` |

Reproduce with `pwsh -File scripts/build-apk.ps1`.

Scope note: this is a faithful reconstruction of the *interaction model and home surface*
(gesture consumers, Shelf card contract, Hidden Space, pager, recents) and not a
reimplementation of the privileged platform integration — the analyzed launcher declares 56
permissions including signature-or-system ones, and is a system HOME app on OxygenOS.
Launcher Lab deliberately does not register as HOME replacement and requests only
`ACCESS_WIFI_STATE` and `BLUETOOTH`.

## 9. Limitations (provider-stated)

- Static decompiler observations do not establish runtime behavior; the APK is never executed.
- Display names may be JADX aliases; overload indices are engine-local; synthetic members included; source positions unknown.
- Reference tracing covers statically-resolved `using` nodes only; reflection/dynamic calls remain unknown.
- Manifest summary fields are extracted by the upstream engine; the app's signature itself was not verified here.

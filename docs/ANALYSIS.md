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

## 7. Limitations (provider-stated)

- Static decompiler observations do not establish runtime behavior; the APK is never executed.
- Display names may be JADX aliases; overload indices are engine-local; synthetic members included; source positions unknown.
- Reference tracing covers statically-resolved `using` nodes only; reflection/dynamic calls remain unknown.
- Manifest summary fields are extracted by the upstream engine; the app's signature itself was not verified here.

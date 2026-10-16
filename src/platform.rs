//! Real device integration — the launcher's "backend".
//!
//! Every surface the user sees must come from the phone itself. This module is
//! the single place where Launcher Lab talks to Android through JNI. It is the
//! production replacement for the static demo catalog in `src/icons.rs`: that
//! catalog survives only as a painter/fallback for a fixed set of well-known
//! packages, never as the source of truth for what is installed.
//!
//! ## What is real, and what cannot be
//!
//! | Surface             | Source of truth                                     |
//! |---------------------|-----------------------------------------------------|
//! | Installed apps      | `PackageManager.queryIntentActivities` (this file)  |
//! | App launching       | `Context.startActivity` (this file)                 |
//! | App icons           | `ApplicationInfo.loadIcon` → `Bitmap` (this file)   |
//! | Wallpaper           | `WallpaperManager.getDrawable` → `Bitmap` (this file)|
//! | Notifications       | reconstructed — needs a NotificationListenerService |
//! | Recents / overview  | reconstructed — true recents need usage access      |
//!
//! ## Error-handling contract
//!
//! * Nothing here panics. Every entry point returns
//!   `Result<_, PlatformError>`; the UI surfaces
//!   [`PlatformError::user_message`] and keeps its previous state.
//! * No Java exception is allowed to cross a JNI boundary. After every
//!   fallible JNI call we check `ExceptionCheck`; a pending exception is
//!   described to logcat and cleared. Leaving one pending would poison every
//!   following JNI call and abort the process when the thread detaches.
//! * Local references are scoped with `PushLocalFrame`/`PopLocalFrame` so even
//!   an error path cannot overflow the JNI local-reference table.
//! * Every value crossing the boundary is validated: component names going
//!   into the JVM, and labels/strings/sizes coming out of it.
//! * On non-Android builds (`cargo test` on a host, the desktop dev window)
//!   everything short-circuits to [`PlatformError::Unsupported`], so this code
//!   still compiles and is exercised without a device.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use jni::objects::{JObject, JString, JValue};
use jni::sys::jint;
use jni::{JNIEnv, JavaVM};

// ---------------------------------------------------------------------------
// Limits and constants
// ---------------------------------------------------------------------------

/// `Intent.FLAG_ACTIVITY_NEW_TASK`.
const INTENT_FLAG_NEW_TASK: jint = 0x1000_0000;
/// `Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED`.
const INTENT_FLAG_RESET_TASK_IF_NEEDED: jint = 0x0020_0000;

const INTENT_ACTION_MAIN: &str = "android.intent.action.MAIN";
const INTENT_CATEGORY_LAUNCHER: &str = "android.intent.category.LAUNCHER";

/// Local-reference headroom requested for every `PushLocalFrame`.
const LOCAL_FRAME_CAPACITY: jint = 64;
/// Local-reference headroom for a single launcher-entry read.
const ENTRY_FRAME_CAPACITY: jint = 16;
/// A phone with more launcher activities than this is not a phone we trust.
const MAX_INSTALLED_APPS: usize = 512;
/// Display labels are truncated to this many characters before reaching the UI.
const MAX_LABEL_CHARS: usize = 128;
/// Icon requests are clamped to this range (physical pixels).
const ICON_MIN_EDGE: u32 = 32;
const ICON_MAX_EDGE: u32 = 512;
/// Wallpaper targets are clamped to this range (physical pixels).
const WALLPAPER_MIN_EDGE: u32 = 64;
const WALLPAPER_MAX_EDGE: jint = 2560;
/// Hard ceiling for any single decoded image (memory safety).
const MAX_IMAGE_PIXELS: i64 = (WALLPAPER_MAX_EDGE as i64) * (WALLPAPER_MAX_EDGE as i64);

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// One real, launchable activity discovered on the device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealApp {
    /// Android package name, e.g. `com.android.chrome`.
    pub package: String,
    /// Fully-qualified launcher activity class, e.g. `com.android.chrome.Main`.
    pub activity: String,
    /// User-facing label (sanitised, never empty), e.g. `Chrome`.
    pub label: String,
}

/// A decoded image (icon or wallpaper) ready for upload as a texture.
///
/// Pixels are unmultiplied RGBA8, exactly what `egui::ColorImage` expects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageRgba {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes, row-major.
    pub rgba: Vec<u8>,
}

/// Everything that can go wrong on the way to real device data.
#[derive(Debug)]
pub enum PlatformError {
    /// Compiled for a host OS, or the Android gate was never opened.
    Unsupported,
    /// Running on Android but the JavaVM/context handles are missing.
    NotInitialized,
    /// Attaching the calling thread to the JVM failed.
    Attach(String),
    /// A specific JNI operation failed (op is a short human-readable phrase).
    Jni { op: &'static str, detail: String },
    /// `getApplicationInfo` reported the package is gone.
    AppNotFound(String),
    /// `startActivity` refused to start the component.
    LaunchFailed(String),
    /// A package/class string failed validation and was blocked before use.
    InvalidComponent(String),
    /// `queryIntentActivities` returned an implausible count; rejected.
    TooManyApps(usize),
    /// Android returned no drawable or a bitmap we refused to decode.
    BadImage(String),
    /// `WallpaperManager.getDrawable` returned null.
    NoWallpaper,
}

impl PlatformError {
    /// A short, user-facing sentence suitable for an in-app toast.
    pub fn user_message(&self) -> String {
        match self {
            PlatformError::Unsupported => {
                "Live device data isn't available in this build — the bundled demo content is shown instead.".to_owned()
            }
            PlatformError::NotInitialized => {
                "The launcher couldn't reach Android's system services. Restart the app; if it keeps happening, reinstall it.".to_owned()
            }
            PlatformError::Attach(_) => {
                "Couldn't connect to the Android runtime. Please try again.".to_owned()
            }
            PlatformError::Jni { op, .. } => {
                format!("Android returned an error while {op}. Nothing was changed — you can try again.")
            }
            PlatformError::AppNotFound(package) => format!("{package} isn't installed anymore."),
            PlatformError::LaunchFailed(name) => {
                format!("Couldn't start {name}. It may have been disabled or removed.")
            }
            PlatformError::InvalidComponent(reason) => {
                format!("Blocked an invalid app reference ({reason}).")
            }
            PlatformError::TooManyApps(count) => format!(
                "Android reported {count} apps, which isn't plausible — the list was left unchanged."
            ),
            PlatformError::BadImage(_) => {
                "Android returned an unusable image — the drawn look is kept instead.".to_owned()
            }
            PlatformError::NoWallpaper => {
                "No system wallpaper is set — the procedural one is used instead.".to_owned()
            }
        }
    }
}

impl fmt::Display for PlatformError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlatformError::Unsupported => write!(formatter, "not running on Android"),
            PlatformError::NotInitialized => write!(formatter, "Android context not initialised"),
            PlatformError::Attach(detail) => write!(formatter, "could not attach to the JVM: {detail}"),
            PlatformError::Jni { op, detail } => write!(formatter, "JNI call failed while {op}: {detail}"),
            PlatformError::AppNotFound(package) => write!(formatter, "app not installed: {package}"),
            PlatformError::LaunchFailed(name) => write!(formatter, "could not launch {name}"),
            PlatformError::InvalidComponent(reason) => write!(formatter, "invalid component: {reason}"),
            PlatformError::TooManyApps(count) => write!(formatter, "implausible app count: {count}"),
            PlatformError::BadImage(detail) => write!(formatter, "unusable image: {detail}"),
            PlatformError::NoWallpaper => write!(formatter, "no system wallpaper"),
        }
    }
}

impl std::error::Error for PlatformError {}

impl From<jni::errors::Error> for PlatformError {
    fn from(error: jni::errors::Error) -> Self {
        PlatformError::Jni {
            op: "talking to Android",
            detail: error.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Android gate
// ---------------------------------------------------------------------------

/// True once [`set_android`] has run (i.e. we are inside a real Android app).
static ANDROID: AtomicBool = AtomicBool::new(false);
/// The process-lifetime JavaVM handle, adopted at startup.
static VM: OnceLock<JavaVM> = OnceLock::new();

/// Whether the process is a live Android app with an adopted JavaVM.
///
/// The UI uses this to decide between real device data and the bundled demo
/// surfaces; on the desktop dev build it is always `false`.
pub fn is_android() -> bool {
    ANDROID.load(Ordering::SeqCst)
}

/// Adopt the live JavaVM. Must be called once, first thing, from
/// `android_main` (after logging is initialised) — never on a host build.
///
/// `android-activity` initialises `ndk-context` with the JavaVM and the
/// activity handle before `android_main` runs, so both are available here.
pub fn set_android() {
    ANDROID.store(true, Ordering::SeqCst);
    #[cfg(target_os = "android")]
    {
        let context = ndk_context::android_context();
        // SAFETY: ndk-context holds a process-lifetime JavaVM pointer that is
        // set before `android_main` and only released at activity teardown.
        match unsafe { JavaVM::from_raw(context.vm().cast()) } {
            Ok(vm) => {
                if VM.set(vm).is_err() {
                    log::debug!("platform: JavaVM was already adopted");
                }
            }
            Err(error) => log::error!("platform: could not adopt the JavaVM: {error}"),
        }
    }
}

/// The adopted JavaVM, or a structured error the UI can show.
fn vm() -> Result<&'static JavaVM, PlatformError> {
    VM.get().ok_or(PlatformError::NotInitialized)
}

/// Run `f` with the calling thread attached to the JVM.
///
/// Attaching is a no-op on threads that are already attached, and the guard
/// detaches (freeing all local references) when we attached it ourselves.
/// A pending Java exception is always described and cleared before the attach
/// scope ends: leaving one pending would abort the process at detach.
fn with_env<T>(
    f: impl FnOnce(&mut JNIEnv<'_>) -> Result<T, PlatformError>,
) -> Result<T, PlatformError> {
    if !is_android() {
        return Err(PlatformError::Unsupported);
    }
    let vm = vm()?;
    let mut thread = vm
        .attach_current_thread()
        .map_err(|error| PlatformError::Attach(error.to_string()))?;
    let result = f(&mut thread);
    let env: &mut JNIEnv<'_> = &mut thread;
    if matches!(env.exception_check(), Ok(true)) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
        if result.is_ok() {
            return Err(PlatformError::Jni {
                op: "talking to Android",
                detail: "an unexpected Java exception was pending after a successful call".to_owned(),
            });
        }
    }
    result
}

/// Raw `android.content.Context` handle from `ndk-context`, or `Unsupported`
/// when running off-device.
#[cfg(target_os = "android")]
fn activity_context_raw() -> Result<*mut std::ffi::c_void, PlatformError> {
    if !is_android() {
        return Err(PlatformError::Unsupported);
    }
    Ok(ndk_context::android_context().context())
}

/// Raw `android.content.Context` handle from `ndk-context`, or `Unsupported`
/// when running off-device.
#[cfg(not(target_os = "android"))]
fn activity_context_raw() -> Result<*mut std::ffi::c_void, PlatformError> {
    Err(PlatformError::Unsupported)
}

/// Run `f` with both the JNI environment and the activity context object.
///
/// The context handle is owned by `android-activity` (a global reference that
/// lives as long as the process), is never deleted here, and is only borrowed
/// for the duration of the call.
fn with_activity_context<T>(
    f: impl FnOnce(&mut JNIEnv<'_>, &JObject<'_>) -> Result<T, PlatformError>,
) -> Result<T, PlatformError> {
    let raw = activity_context_raw()?;
    if raw.is_null() {
        return Err(PlatformError::NotInitialized);
    }
    // SAFETY: on Android, ndk-context guarantees this global reference for the
    // app's whole lifetime; we borrow it for one JNI call chain and never
    // delete it. Off-device this function has already returned `Unsupported`.
    let activity = unsafe { JObject::from_raw(raw as jni::sys::jobject) };
    with_env(|env| f(env, &activity))
}

// ---------------------------------------------------------------------------
// JNI helpers
// ---------------------------------------------------------------------------

/// Convert a failed JNI call into [`PlatformError::Jni`], clearing any pending
/// Java exception first (logcat gets the full descriptor via `ExceptionDescribe`).
fn jni_fail(env: &JNIEnv<'_>, op: &'static str, error: jni::errors::Error) -> PlatformError {
    let pending = matches!(&error, jni::errors::Error::JavaException)
        || matches!(env.exception_check(), Ok(true));
    if pending {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
    let detail = error.to_string();
    log::warn!("platform: {op} failed: {detail}");
    PlatformError::Jni { op, detail }
}

/// Reject JNI calls that returned `null` where a real object is required.
fn ensure_not_null<'local>(
    op: &'static str,
    object: JObject<'local>,
) -> Result<JObject<'local>, PlatformError> {
    if object.is_null() {
        Err(PlatformError::Jni {
            op,
            detail: "Android returned null".to_owned(),
        })
    } else {
        Ok(object)
    }
}

/// `env.new_string` with structured error handling.
fn new_string_ref<'local>(
    env: &mut JNIEnv<'local>,
    value: &str,
) -> Result<JString<'local>, PlatformError> {
    env.new_string(value)
        .map_err(|error| jni_fail(env, "creating a Java string", error))
}

/// `env.new_object` with structured error handling.
fn new_object_ref<'local>(
    env: &mut JNIEnv<'local>,
    op: &'static str,
    class: &str,
    constructor: &str,
    args: &[JValue<'_, '_>],
) -> Result<JObject<'local>, PlatformError> {
    env.new_object(class, constructor, args)
        .map_err(|error| jni_fail(env, op, error))
}

/// Call an instance method that returns an object.
fn call_object<'local>(
    env: &mut JNIEnv<'local>,
    op: &'static str,
    object: &JObject<'_>,
    method: &str,
    signature: &str,
    args: &[JValue<'_, '_>],
) -> Result<JObject<'local>, PlatformError> {
    let call = env.call_method(object, method, signature, args);
    let value = call.map_err(|error| jni_fail(env, op, error))?;
    value.l().map_err(|error| PlatformError::Jni {
        op,
        detail: error.to_string(),
    })
}

/// Call a static method that returns an object.
fn call_static_object<'local>(
    env: &mut JNIEnv<'local>,
    op: &'static str,
    class: &str,
    method: &str,
    signature: &str,
    args: &[JValue<'_, '_>],
) -> Result<JObject<'local>, PlatformError> {
    let call = env.call_static_method(class, method, signature, args);
    let value = call.map_err(|error| jni_fail(env, op, error))?;
    value.l().map_err(|error| PlatformError::Jni {
        op,
        detail: error.to_string(),
    })
}

/// Call an instance method that returns `int`.
fn call_int(
    env: &mut JNIEnv<'_>,
    op: &'static str,
    object: &JObject<'_>,
    method: &str,
    signature: &str,
    args: &[JValue<'_, '_>],
) -> Result<jint, PlatformError> {
    let call = env.call_method(object, method, signature, args);
    let value = call.map_err(|error| jni_fail(env, op, error))?;
    value.i().map_err(|error| PlatformError::Jni {
        op,
        detail: error.to_string(),
    })
}

/// Call an instance method and discard its return value.
fn call_void(
    env: &mut JNIEnv<'_>,
    op: &'static str,
    object: &JObject<'_>,
    method: &str,
    signature: &str,
    args: &[JValue<'_, '_>],
) -> Result<(), PlatformError> {
    let call = env.call_method(object, method, signature, args);
    call.map_err(|error| jni_fail(env, op, error))
        .map(|_value| ())
}

/// Copy a `java.lang.String` local reference into an owned Rust `String` and
/// delete the reference.
fn take_string(env: &mut JNIEnv<'_>, value: JObject<'_>) -> Result<String, PlatformError> {
    let text = {
        let as_string: &JString = (&value).into();
        let java_string = env
            .get_string(as_string)
            .map_err(|error| jni_fail(env, "reading Android strings", error))?;
        String::from(java_string)
    };
    let _ = env.delete_local_ref(value);
    Ok(text)
}

/// Read a `String` field of `object` (e.g. `ActivityInfo.packageName`).
fn field_string(
    env: &mut JNIEnv<'_>,
    object: &JObject<'_>,
    field: &str,
    op: &'static str,
) -> Result<String, PlatformError> {
    let fetched = env.get_field(object, field, "Ljava/lang/String;");
    let value = fetched.map_err(|error| jni_fail(env, op, error))?;
    let value = value.l().map_err(|error| PlatformError::Jni {
        op,
        detail: error.to_string(),
    })?;
    take_string(env, value)
}

/// Read an object field of `object` (e.g. `ResolveInfo.activityInfo`).
fn field_object<'local>(
    env: &mut JNIEnv<'local>,
    object: &JObject<'_>,
    field: &str,
    signature: &str,
    op: &'static str,
) -> Result<JObject<'local>, PlatformError> {
    let fetched = env.get_field(object, field, signature);
    let value = fetched.map_err(|error| jni_fail(env, op, error))?;
    value.l().map_err(|error| PlatformError::Jni {
        op,
        detail: error.to_string(),
    })
}

/// `CharSequence.toString()` into an owned Rust `String`, freeing both refs.
fn char_sequence_to_string(
    env: &mut JNIEnv<'_>,
    sequence: JObject<'_>,
) -> Result<String, PlatformError> {
    let call = env.call_method(&sequence, "toString", "()Ljava/lang/String;", &[]);
    let result = match call {
        Ok(value) => value
            .l()
            .map_err(|error| PlatformError::Jni {
                op: "reading an app label",
                detail: error.to_string(),
            })
            .and_then(|text| take_string(env, text)),
        Err(error) => Err(jni_fail(env, "reading an app label", error)),
    };
    let _ = env.delete_local_ref(sequence);
    result
}

/// Read `ResolveInfo.loadLabel(packageManager)`, falling back to `None` so the
/// caller can substitute the package name.
fn read_label(
    env: &mut JNIEnv<'_>,
    item: &JObject<'_>,
    manager: &JObject<'_>,
) -> Result<String, PlatformError> {
    let call = env.call_method(
        item,
        "loadLabel",
        "(Landroid/content/pm/PackageManager;)Ljava/lang/CharSequence;",
        &[JValue::Object(manager)],
    );
    let sequence = call
        .map_err(|error| jni_fail(env, "reading an app label", error))?
        .l()
        .map_err(|error| PlatformError::Jni {
            op: "reading an app label",
            detail: error.to_string(),
        })?;
    char_sequence_to_string(env, sequence)
}

/// Render any `Drawable` (bitmap, adaptive icon, colour) into a fixed-size
/// `ImageRgba` via `Bitmap.createBitmap` + `Canvas.draw`.
fn drawable_to_image(
    env: &mut JNIEnv<'_>,
    drawable: &JObject<'_>,
    width: jint,
    height: jint,
) -> Result<ImageRgba, PlatformError> {
    let pixels = pixel_count(width, height)?;
    let config = env
        .get_static_field(
            "android/graphics/Bitmap$Config",
            "ARGB_8888",
            "Landroid/graphics/Bitmap$Config;",
        )
        .map_err(|error| jni_fail(env, "decoding an image", error))?
        .l()
        .map_err(|error| PlatformError::Jni {
            op: "decoding an image",
            detail: error.to_string(),
        })?;
    let bitmap = call_static_object(
        env,
        "decoding an image",
        "android/graphics/Bitmap",
        "createBitmap",
        "(IILandroid/graphics/Bitmap$Config;)Landroid/graphics/Bitmap;",
        &[
            JValue::Int(width),
            JValue::Int(height),
            JValue::Object(&config),
        ],
    )?;
    let canvas = new_object_ref(
        env,
        "decoding an image",
        "android/graphics/Canvas",
        "(Landroid/graphics/Bitmap;)V",
        &[JValue::Object(&bitmap)],
    )?;
    call_void(
        env,
        "decoding an image",
        drawable,
        "setBounds",
        "(IIII)V",
        &[
            JValue::Int(0),
            JValue::Int(0),
            JValue::Int(width),
            JValue::Int(height),
        ],
    )?;
    call_void(
        env,
        "decoding an image",
        drawable,
        "draw",
        "(Landroid/graphics/Canvas;)V",
        &[JValue::Object(&canvas)],
    )?;
    let buffer = env
        .new_int_array(pixels as jint)
        .map_err(|error| jni_fail(env, "allocating an image buffer", error))?;
    call_void(
        env,
        "reading image pixels",
        &bitmap,
        "getPixels",
        "([IIIIIII)V",
        &[
            JValue::Object(&*buffer),
            JValue::Int(0),
            JValue::Int(width),
            JValue::Int(0),
            JValue::Int(0),
            JValue::Int(width),
            JValue::Int(height),
        ],
    )?;
    let mut argb = vec![0_i32; pixels];
    env.get_int_array_region(&buffer, 0, &mut argb)
        .map_err(|error| jni_fail(env, "reading image pixels", error))?;
    Ok(ImageRgba {
        width: width as u32,
        height: height as u32,
        rgba: argb_to_rgba(&argb),
    })
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Every launcher activity on the device, sorted by label.
///
/// This is the drawer's source of truth. The launcher's own package is
/// excluded, malformed entries are skipped, and the result is normalised by
/// `normalize_and_sort`.
pub fn query_launcher_apps() -> Result<Vec<RealApp>, PlatformError> {
    with_activity_context(|env, context| {
        env.with_local_frame(LOCAL_FRAME_CAPACITY, |env| {
            let manager = ensure_not_null(
                "reading the package manager",
                call_object(
                    env,
                    "reading the package manager",
                    context,
                    "getPackageManager",
                    "()Landroid/content/pm/PackageManager;",
                    &[],
                )?,
            )?;
            let own_package = match call_object(
                env,
                "reading the launcher's own package name",
                context,
                "getPackageName",
                "()Ljava/lang/String;",
                &[],
            ) {
                Ok(name) => take_string(env, name).unwrap_or_default(),
                Err(error) => {
                    log::warn!("platform: could not read the launcher's own package name: {error}");
                    String::new()
                }
            };

            let action = new_string_ref(env, INTENT_ACTION_MAIN)?;
            let category = new_string_ref(env, INTENT_CATEGORY_LAUNCHER)?;
            let intent = ensure_not_null(
                "building the launcher query",
                new_object_ref(
                    env,
                    "building the launcher query",
                    "android/content/Intent",
                    "(Ljava/lang/String;)V",
                    &[JValue::Object(&*action)],
                )?,
            )?;
            call_void(
                env,
                "building the launcher query",
                &intent,
                "addCategory",
                "(Ljava/lang/String;)Landroid/content/Intent;",
                &[JValue::Object(&*category)],
            )?;
            let listed = ensure_not_null(
                "querying installed apps",
                call_object(
                    env,
                    "querying installed apps",
                    &manager,
                    "queryIntentActivities",
                    "(Landroid/content/Intent;I)Ljava/util/List;",
                    &[JValue::Object(&intent), JValue::Int(0)],
                )?,
            )?;
            let count = call_int(env, "querying installed apps", &listed, "size", "()I", &[])?;
            if count < 0 || count as usize > MAX_INSTALLED_APPS {
                return Err(PlatformError::TooManyApps(count.max(0) as usize));
            }

            let mut apps: Vec<RealApp> = Vec::with_capacity(count as usize);
            for index in 0..count {
                // A fresh local frame per entry keeps reference usage flat for
                // the whole loop even when individual entries fail.
                let entry = env.with_local_frame(ENTRY_FRAME_CAPACITY, |env| {
                    read_launcher_entry(env, &listed, &manager, index, &own_package)
                });
                match entry {
                    Ok(Some(app)) => apps.push(app),
                    Ok(None) => {}
                    Err(error) => log::warn!("platform: skipping launcher entry #{index}: {error}"),
                }
            }
            normalize_and_sort(&mut apps, &own_package);
            Ok(apps)
        })
    })
}

/// Read one `ResolveInfo` from the query result.
///
/// Returns `Ok(None)` for entries that are empty, are the launcher itself, or
/// have malformed package/class names (validated before they can reach the
/// launch path). A missing label degrades to the package name.
fn read_launcher_entry(
    env: &mut JNIEnv<'_>,
    list: &JObject<'_>,
    manager: &JObject<'_>,
    index: jint,
    own_package: &str,
) -> Result<Option<RealApp>, PlatformError> {
    let item = call_object(
        env,
        "reading an app entry",
        list,
        "get",
        "(I)Ljava/lang/Object;",
        &[JValue::Int(index)],
    )?;
    let info = field_object(
        env,
        &item,
        "activityInfo",
        "Landroid/content/pm/ActivityInfo;",
        "reading an app entry",
    )?;
    let package = field_string(env, &info, "packageName", "reading an app entry")?;
    let activity = field_string(env, &info, "name", "reading an app entry")?;

    if package.is_empty() || activity.is_empty() || package == own_package {
        return Ok(None);
    }
    if validate_package(&package).is_err() || package_problem_activity(&activity).is_some() {
        log::warn!("platform: skipping malformed entry {package}/{activity}");
        return Ok(None);
    }

    let label = match read_label(env, &item, manager) {
        Ok(label) => label,
        Err(error) => {
            log::warn!("platform: label lookup failed for {package}: {error}");
            String::new()
        }
    };
    Ok(Some(RealApp {
        label: sanitize_label(&label, &package),
        package,
        activity,
    }))
}

/// Launch a real app activity via `Context.startActivity`.
///
/// The component is validated first; the intent carries `NEW_TASK` +
/// `RESET_TASK_IF_NEEDED` like a stock launcher. Failures map to
/// [`PlatformError::LaunchFailed`] so the UI can explain what happened.
pub fn launch_app(app: &RealApp) -> Result<(), PlatformError> {
    validate_component(&app.package, &app.activity)?;
    with_activity_context(|env, context| {
        env.with_local_frame(LOCAL_FRAME_CAPACITY, |env| {
            let package = new_string_ref(env, &app.package)?;
            let activity = new_string_ref(env, &app.activity)?;
            let component = new_object_ref(
                env,
                "starting the app",
                "android/content/ComponentName",
                "(Ljava/lang/String;Ljava/lang/String;)V",
                &[JValue::Object(&*package), JValue::Object(&*activity)],
            )?;
            let intent = new_object_ref(
                env,
                "starting the app",
                "android/content/Intent",
                "()V",
                &[],
            )?;
            call_void(
                env,
                "starting the app",
                &intent,
                "setComponent",
                "(Landroid/content/ComponentName;)Landroid/content/Intent;",
                &[JValue::Object(&component)],
            )?;
            call_void(
                env,
                "starting the app",
                &intent,
                "setFlags",
                "(I)Landroid/content/Intent;",
                &[JValue::Int(
                    INTENT_FLAG_NEW_TASK | INTENT_FLAG_RESET_TASK_IF_NEEDED,
                )],
            )?;
            let started = env.call_method(
                context,
                "startActivity",
                "(Landroid/content/Intent;)V",
                &[JValue::Object(&intent)],
            );
            match started {
                Ok(_) => Ok(()),
                Err(error) => {
                    let failure = jni_fail(env, "starting the app", error);
                    log::warn!("platform: launch of {} failed: {failure}", app.package);
                    Err(PlatformError::LaunchFailed(format!(
                        "{} ({})",
                        app.label, app.package
                    )))
                }
            }
        })
    })
}

/// Load the real icon of `package` as unmultiplied RGBA8.
///
/// `requested_size` is in physical pixels and is clamped; the icon is rendered
/// through a `Canvas` so adaptive icons (API 26+) work too.
pub fn app_icon_rgba(package: &str, requested_size: u32) -> Result<ImageRgba, PlatformError> {
    validate_package(package)?;
    let size = clamp_icon_size(requested_size);
    with_activity_context(|env, context| {
        env.with_local_frame(LOCAL_FRAME_CAPACITY, |env| {
            let manager = call_object(
                env,
                "loading the app icon",
                context,
                "getPackageManager",
                "()Landroid/content/pm/PackageManager;",
                &[],
            )?;
            let name = new_string_ref(env, package)?;
            let info_call = env.call_method(
                &manager,
                "getApplicationInfo",
                "(Ljava/lang/String;I)Landroid/content/pm/ApplicationInfo;",
                &[JValue::Object(&*name), JValue::Int(0)],
            );
            let info = match info_call {
                Ok(value) => value.l().map_err(|error| PlatformError::Jni {
                    op: "loading the app icon",
                    detail: error.to_string(),
                })?,
                Err(error) => {
                    let _ = jni_fail(env, "loading the app icon", error);
                    return Err(PlatformError::AppNotFound(package.to_owned()));
                }
            };
            let icon_call = env.call_method(
                &info,
                "loadIcon",
                "(Landroid/content/pm/PackageManager;)Landroid/graphics/drawable/Drawable;",
                &[JValue::Object(&manager)],
            );
            let drawable = icon_call
                .map_err(|error| jni_fail(env, "loading the app icon", error))?
                .l()
                .map_err(|error| PlatformError::Jni {
                    op: "loading the app icon",
                    detail: error.to_string(),
                })?;
            if drawable.is_null() {
                return Err(PlatformError::BadImage(format!("{package} has no icon")));
            }
            drawable_to_image(env, &drawable, size, size)
        })
    })
}

/// The live system wallpaper, scaled to cover `target_width` x `target_height`
/// physical pixels (overscan for parallax happens in the UI layer).
pub fn system_wallpaper_rgba(
    target_width: u32,
    target_height: u32,
) -> Result<ImageRgba, PlatformError> {
    let (width, height) = clamp_wallpaper_targets(target_width, target_height);
    with_activity_context(|env, context| {
        env.with_local_frame(LOCAL_FRAME_CAPACITY, |env| {
            let manager = call_static_object(
                env,
                "reading the system wallpaper",
                "android/app/WallpaperManager",
                "getInstance",
                "(Landroid/content/Context;)Landroid/app/WallpaperManager;",
                &[JValue::Object(context)],
            )?;
            let drawable = call_object(
                env,
                "reading the system wallpaper",
                &manager,
                "getDrawable",
                "()Landroid/graphics/drawable/Drawable;",
                &[],
            )?;
            if drawable.is_null() {
                return Err(PlatformError::NoWallpaper);
            }
            // Unknown intrinsic size simply means "draw at the target size".
            let intrinsic_width = call_int(
                env,
                "reading the system wallpaper",
                &drawable,
                "getIntrinsicWidth",
                "()I",
                &[],
            )
            .unwrap_or(-1);
            let intrinsic_height = call_int(
                env,
                "reading the system wallpaper",
                &drawable,
                "getIntrinsicHeight",
                "()I",
                &[],
            )
            .unwrap_or(-1);
            let (draw_width, draw_height) = cover_dims(
                intrinsic_width,
                intrinsic_height,
                width as jint,
                height as jint,
                WALLPAPER_MAX_EDGE,
            );
            drawable_to_image(env, &drawable, draw_width, draw_height)
        })
    })
}

/// A live snapshot of the system toggles the shade reports.
///
/// Every field is best-effort: `None` means the value could not be read on
/// this device/OS version (missing permission, reserved API, ...) and the UI
/// shows the toggle as unknown instead of inventing a state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SystemState {
    pub wifi: Option<bool>,
    pub bluetooth: Option<bool>,
    pub airplane: Option<bool>,
    pub auto_rotate: Option<bool>,
    pub battery_saver: Option<bool>,
}

/// Read the live state of Wi-Fi, Bluetooth, airplane mode, auto-rotate and
/// battery saver. Never fails wholesale: unreadable values come back `None`.
pub fn read_system_states() -> Result<SystemState, PlatformError> {
    with_activity_context(|env, context| {
        env.with_local_frame(LOCAL_FRAME_CAPACITY, |env| {
            let mut state = SystemState::default();
            state.wifi = read_wifi_state(env, context);
            state.bluetooth = read_bluetooth_state(env, context);
            state.airplane = read_bool_setting(
                env,
                context,
                "android/provider/Settings$Global",
                "airplane_mode_on",
            );
            state.auto_rotate = read_bool_setting(
                env,
                context,
                "android/provider/Settings$System",
                "accelerometer_rotation",
            );
            state.battery_saver = read_power_saver(env, context);
            Ok(state)
        })
    })
}

/// Map a shade toggle key to the system settings action it opens.
///
/// A normal app cannot flip Wi-Fi/Bluetooth/etc. on Android 10+, so the tiles
/// open the real settings screens instead of faking a toggle.
pub fn system_screen_action(key: &str) -> Option<&'static str> {
    match key {
        "wifi" => Some("android.settings.WIFI_SETTINGS"),
        "bluetooth" => Some("android.settings.BLUETOOTH_SETTINGS"),
        "airplane" => Some("android.settings.AIRPLANE_MODE_SETTINGS"),
        "display" => Some("android.settings.DISPLAY_SETTINGS"),
        "battery_saver" => Some("android.settings.BATTERY_SAVER_SETTINGS"),
        _ => None,
    }
}

/// Open one of the whitelisted system settings screens.
pub fn open_system_screen(key: &str) -> Result<(), PlatformError> {
    let action = system_screen_action(key).ok_or_else(|| {
        PlatformError::InvalidComponent(format!("unknown system screen {key:?}"))
    })?;
    with_activity_context(|env, context| {
        env.with_local_frame(ENTRY_FRAME_CAPACITY, |env| {
            let action = new_string_ref(env, action)?;
            let intent = new_object_ref(
                env,
                "opening a settings screen",
                "android/content/Intent",
                "(Ljava/lang/String;)V",
                &[JValue::Object(&*action)],
            )?;
            call_void(
                env,
                "opening a settings screen",
                &intent,
                "setFlags",
                "(I)Landroid/content/Intent;",
                &[JValue::Int(INTENT_FLAG_NEW_TASK)],
            )?;
            let started = env.call_method(
                context,
                "startActivity",
                "(Landroid/content/Intent;)V",
                &[JValue::Object(&intent)],
            );
            match started {
                Ok(_) => Ok(()),
                Err(error) => {
                    let failure = jni_fail(env, "opening a settings screen", error);
                    log::warn!("platform: opening the {key} screen failed: {failure}");
                    Err(PlatformError::LaunchFailed(format!(
                        "the {key} settings screen"
                    )))
                }
            }
        })
    })
}

/// `WifiManager.isWifiEnabled()` (`ACCESS_WIFI_STATE` is a normal permission).
fn read_wifi_state(env: &mut JNIEnv<'_>, context: &JObject<'_>) -> Option<bool> {
    let service = new_string_ref(env, "wifi").ok()?;
    let manager = call_object(
        env,
        "reading Wi-Fi state",
        context,
        "getSystemService",
        "(Ljava/lang/String;)Ljava/lang/Object;",
        &[JValue::Object(&*service)],
    )
    .ok()?;
    if manager.is_null() {
        return None;
    }
    match env.call_method(&manager, "isWifiEnabled", "()Z", &[]) {
        Ok(value) => value.z().ok(),
        Err(error) => {
            let _ = jni_fail(env, "reading Wi-Fi state", error);
            None
        }
    }
}

/// `BluetoothAdapter.getState() == STATE_ON` (legacy BLUETOOTH permission;
/// newer devices without the runtime grant report `None`).
fn read_bluetooth_state(env: &mut JNIEnv<'_>, context: &JObject<'_>) -> Option<bool> {
    let service = new_string_ref(env, "bluetooth").ok()?;
    let manager = call_object(
        env,
        "reading Bluetooth state",
        context,
        "getSystemService",
        "(Ljava/lang/String;)Ljava/lang/Object;",
        &[JValue::Object(&*service)],
    )
    .ok()?;
    if manager.is_null() {
        return None;
    }
    let adapter = call_object(
        env,
        "reading Bluetooth state",
        &manager,
        "getAdapter",
        "()Landroid/bluetooth/BluetoothAdapter;",
        &[],
    )
    .ok()?;
    if adapter.is_null() {
        return None;
    }
    match env.call_method(&adapter, "getState", "()I", &[]) {
        Ok(value) => value.i().ok().map(|state| state == 12), // STATE_ON
        Err(error) => {
            let _ = jni_fail(env, "reading Bluetooth state", error);
            None
        }
    }
}

/// `Settings.Global/System.getInt(resolver, name, 0) != 0`.
fn read_bool_setting(
    env: &mut JNIEnv<'_>,
    context: &JObject<'_>,
    settings_class: &str,
    name: &str,
) -> Option<bool> {
    let resolver = call_object(
        env,
        "reading a system setting",
        context,
        "getContentResolver",
        "()Landroid/content/ContentResolver;",
        &[],
    )
    .ok()?;
    if resolver.is_null() {
        return None;
    }
    let key = new_string_ref(env, name).ok()?;
    let call = env.call_static_method(
        settings_class,
        "getInt",
        "(Landroid/content/ContentResolver;Ljava/lang/String;I)I",
        &[
            JValue::Object(&resolver),
            JValue::Object(&*key),
            JValue::Int(0),
        ],
    );
    match call {
        Ok(value) => value.i().ok().map(|setting| setting != 0),
        Err(error) => {
            let _ = jni_fail(env, "reading a system setting", error);
            None
        }
    }
}

/// `PowerManager.isPowerSaveMode()` (public, no permission).
fn read_power_saver(env: &mut JNIEnv<'_>, context: &JObject<'_>) -> Option<bool> {
    let service = new_string_ref(env, "power").ok()?;
    let manager = call_object(
        env,
        "reading battery-saver state",
        context,
        "getSystemService",
        "(Ljava/lang/String;)Ljava/lang/Object;",
        &[JValue::Object(&*service)],
    )
    .ok()?;
    if manager.is_null() {
        return None;
    }
    match env.call_method(&manager, "isPowerSaveMode", "()Z", &[]) {
        Ok(value) => value.z().ok(),
        Err(error) => {
            let _ = jni_fail(env, "reading battery-saver state", error);
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Validation and pure helpers (host-testable)
// ---------------------------------------------------------------------------

/// A Java identifier segment: starts with a letter or `_`, then letters,
/// digits, `_` or `$`.
fn is_java_identifier(segment: &str) -> bool {
    let mut chars = segment.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// Why `activity` is not a valid launcher component class, if it isn't.
fn package_problem_activity(activity: &str) -> Option<String> {
    if activity.is_empty() {
        return Some("the class name is empty".to_owned());
    }
    if activity.len() > 255 {
        return Some("the class name is longer than 255 bytes".to_owned());
    }
    if !activity.contains('.') {
        return Some("the class name is not fully qualified".to_owned());
    }
    if !activity.split('.').all(is_java_identifier) {
        return Some("the class name is not valid Java".to_owned());
    }
    None
}

/// Validate a package name coming from (or going into) the platform layer.
pub fn validate_package(package: &str) -> Result<(), PlatformError> {
    if package.is_empty() {
        return Err(PlatformError::InvalidComponent(
            "the package name is empty".to_owned(),
        ));
    }
    if package.len() > 255 {
        return Err(PlatformError::InvalidComponent(
            "the package name is longer than 255 bytes".to_owned(),
        ));
    }
    if !package.contains('.') {
        return Err(PlatformError::InvalidComponent(
            "the package name is not fully qualified".to_owned(),
        ));
    }
    if !package.split('.').all(is_java_identifier) {
        return Err(PlatformError::InvalidComponent(
            "the package name is not valid Java".to_owned(),
        ));
    }
    Ok(())
}

/// Validate a package/activity pair before it can reach `ComponentName`.
pub fn validate_component(package: &str, activity: &str) -> Result<(), PlatformError> {
    validate_package(package)?;
    match package_problem_activity(activity) {
        Some(problem) => Err(PlatformError::InvalidComponent(problem)),
        None => Ok(()),
    }
}

/// Make an Android display label safe for the UI.
///
/// Control characters become spaces, the text is trimmed and capped at
/// [`MAX_LABEL_CHARS`] characters, and an empty result falls back to the last
/// package-name segment (or `App`).
pub fn sanitize_label(raw: &str, fallback: &str) -> String {
    let flattened: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let trimmed = flattened.trim();
    let capped: String = trimmed.chars().take(MAX_LABEL_CHARS).collect();
    if !capped.trim().is_empty() {
        return capped;
    }
    let segment = fallback.rsplit('.').next().unwrap_or("");
    if segment.is_empty() {
        "App".to_owned()
    } else {
        segment.to_owned()
    }
}

/// Filter out the own package and malformed rows, de-duplicate by
/// (package, activity), then sort case-insensitively by label.
pub fn normalize_and_sort(apps: &mut Vec<RealApp>, own_package: &str) {
    apps.retain(|app| {
        app.package != own_package
            && !app.package.is_empty()
            && !app.activity.is_empty()
            && !app.label.is_empty()
    });
    apps.sort_by(|left, right| {
        left.label
            .to_lowercase()
            .cmp(&right.label.to_lowercase())
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.package.cmp(&right.package))
            .then_with(|| left.activity.cmp(&right.activity))
    });
    apps.dedup_by(|left, right| {
        left.package == right.package && left.activity == right.activity
    });
}

/// Android `getPixels` values (non-premultiplied ARGB ints) to RGBA8 bytes.
pub fn argb_to_rgba(pixels: &[i32]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(pixels.len() * 4);
    for &pixel in pixels {
        let value = pixel as u32;
        rgba.push(((value >> 16) & 0xFF) as u8);
        rgba.push(((value >> 8) & 0xFF) as u8);
        rgba.push((value & 0xFF) as u8);
        rgba.push(((value >> 24) & 0xFF) as u8);
    }
    rgba
}

/// Clamp a requested icon edge to the supported physical-pixel range.
pub fn clamp_icon_size(requested: u32) -> jint {
    requested.clamp(ICON_MIN_EDGE, ICON_MAX_EDGE) as jint
}

/// Clamp requested wallpaper targets to the supported physical-pixel range.
pub fn clamp_wallpaper_targets(width: u32, height: u32) -> (u32, u32) {
    (
        width.clamp(WALLPAPER_MIN_EDGE, WALLPAPER_MAX_EDGE as u32),
        height.clamp(WALLPAPER_MIN_EDGE, WALLPAPER_MAX_EDGE as u32),
    )
}

/// Dimensions that scale the source to cover the target box without
/// distortion, capped at `max_edge` (unknown/<=0 intrinsic => target size).
pub fn cover_dims(
    intrinsic_width: jint,
    intrinsic_height: jint,
    target_width: jint,
    target_height: jint,
    max_edge: jint,
) -> (jint, jint) {
    let max_edge = i64::from(max_edge.max(1));
    let target_width = target_width.max(1).min(max_edge as jint);
    let target_height = target_height.max(1).min(max_edge as jint);
    if intrinsic_width <= 0 || intrinsic_height <= 0 {
        return (target_width, target_height);
    }
    let scale = f64::max(
        target_width as f64 / intrinsic_width as f64,
        target_height as f64 / intrinsic_height as f64,
    );
    let mut width = (intrinsic_width as f64 * scale).ceil() as i64;
    let mut height = (intrinsic_height as f64 * scale).ceil() as i64;
    if width > max_edge || height > max_edge {
        let shrink = (max_edge as f64 / width as f64).min(max_edge as f64 / height as f64);
        width = (width as f64 * shrink).floor() as i64;
        height = (height as f64 * shrink).floor() as i64;
    }
    (
        width.clamp(1, max_edge) as jint,
        height.clamp(1, max_edge) as jint,
    )
}

/// Pixel budget check for any decoded image; rejects non-positive and
/// oversized requests before a bitmap is ever allocated.
pub fn pixel_count(width: jint, height: jint) -> Result<usize, PlatformError> {
    if width <= 0 || height <= 0 {
        return Err(PlatformError::BadImage(format!(
            "non-positive size {width}x{height}"
        )));
    }
    let total = i64::from(width) * i64::from(height);
    if total > MAX_IMAGE_PIXELS {
        return Err(PlatformError::BadImage(format!(
            "refusing to decode {width}x{height}"
        )));
    }
    Ok(total as usize)
}

// ---------------------------------------------------------------------------
// Tests (host build: no JNI, no device)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn app(package: &str, activity: &str, label: &str) -> RealApp {
        RealApp {
            package: package.to_owned(),
            activity: activity.to_owned(),
            label: label.to_owned(),
        }
    }

    #[test]
    fn package_validation() {
        assert!(validate_package("com.android.chrome").is_ok());
        assert!(validate_package("net.oneplus.launcher").is_ok());
        assert!(validate_package("com.example_app.game1").is_ok());
        for bad in [
            "",
            "nodots",
            ".leading",
            "trailing.",
            "com..double",
            "com.exa mple",
            "com.9start",
            "com.ex-ample",
        ] {
            assert!(validate_package(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn component_validation() {
        assert!(validate_component("com.android.chrome", "com.google.android.apps.chrome.Main").is_ok());
        assert!(validate_component("com.android.chrome", "com.google.android.apps.chrome.Main$Alias").is_ok());
        assert!(validate_component("", "com.example.Main").is_err());
        assert!(validate_component("com.example.app", "").is_err());
        assert!(validate_component("com.example.app", "MainActivity").is_err());
        assert!(validate_component("com.example.app", "com.example.9Bad").is_err());
    }

    #[test]
    fn label_sanitisation() {
        assert_eq!(sanitize_label("  Chrome  ", "com.android.chrome"), "Chrome");
        assert_eq!(sanitize_label("Line\nTwo", "com.example.app"), "Line Two");
        assert_eq!(sanitize_label("", "com.android.chrome"), "chrome");
        assert_eq!(sanitize_label("   ", ""), "App");
        assert_eq!(
            sanitize_label(&"x".repeat(500), "com.example.app").chars().count(),
            MAX_LABEL_CHARS
        );
    }

    #[test]
    fn normalization_filters_and_sorts() {
        let mut apps = vec![
            app("com.zeta.player", "com.zeta.player.Main", "Zeta"),
            app("dev.rajbhx.launcherlab", "dev.rajbhx.launcherlab.MainActivity", "Launcher Lab"),
            app("com.alpha.chat", "com.alpha.chat.Home", "alpha chat"),
            app("com.alpha.chat", "com.alpha.chat.Home", "Alpha Chat"),
            app("com.beta.maps", "com.beta.maps.MapActivity", "Beta Maps"),
            app("", "com.empty.Main", "Empty"),
        ];
        normalize_and_sort(&mut apps, "dev.rajbhx.launcherlab");
        let labels: Vec<&str> = apps.iter().map(|entry| entry.label.as_str()).collect();
        assert_eq!(labels, vec!["Alpha Chat", "Beta Maps", "Zeta"]);
    }

    #[test]
    fn normalization_is_deterministic_on_ties() {
        let mut apps = vec![
            app("com.b.two", "com.b.two.M", "same"),
            app("com.a.one", "com.a.one.M", "Same"),
        ];
        normalize_and_sort(&mut apps, "");
        let labels: Vec<&str> = apps.iter().map(|entry| entry.label.as_str()).collect();
        assert_eq!(labels, vec!["Same", "same"]);
    }

    #[test]
    fn pixels_convert_to_rgba() {
        assert_eq!(argb_to_rgba(&[0x8040_2010u32 as i32]), vec![0x40, 0x20, 0x10, 0x80]);
        assert_eq!(argb_to_rgba(&[]), Vec::<u8>::new());
        assert_eq!(argb_to_rgba(&[0xFFFF_FFFFu32 as i32]), vec![0xFF, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn size_clamps() {
        assert_eq!(clamp_icon_size(0), ICON_MIN_EDGE as jint);
        assert_eq!(clamp_icon_size(144), 144);
        assert_eq!(clamp_icon_size(9_999), ICON_MAX_EDGE as jint);
        assert_eq!(
            clamp_wallpaper_targets(0, 9_000),
            (WALLPAPER_MIN_EDGE, WALLPAPER_MAX_EDGE as u32)
        );
    }

    #[test]
    fn cover_scaling() {
        let (width, height) = cover_dims(1080, 2160, 1080, 2340, WALLPAPER_MAX_EDGE);
        assert!((1169..=1171).contains(&width), "width was {width}");
        assert!((2339..=2341).contains(&height), "height was {height}");
        assert_eq!(cover_dims(-1, -1, 1080, 2340, WALLPAPER_MAX_EDGE), (1080, 2340));
        let (capped_width, capped_height) = cover_dims(100, 100_000, 1080, 2340, WALLPAPER_MAX_EDGE);
        assert!((1..=WALLPAPER_MAX_EDGE).contains(&capped_width));
        assert!((1..=WALLPAPER_MAX_EDGE).contains(&capped_height));
    }

    #[test]
    fn pixel_budget() {
        assert!(pixel_count(0, 100).is_err());
        assert!(pixel_count(100, 0).is_err());
        assert!(pixel_count(-4, 100).is_err());
        assert_eq!(pixel_count(2560, 2560).unwrap() as i64, MAX_IMAGE_PIXELS);
        assert!(pixel_count(2560, 2561).is_err());
    }

    #[test]
    fn host_builds_report_unsupported() {
        assert!(!is_android());
        assert!(matches!(
            query_launcher_apps(),
            Err(PlatformError::Unsupported)
        ));
        let valid = app("com.example.app", "com.example.app.Main", "Example");
        assert!(matches!(
            launch_app(&valid),
            Err(PlatformError::Unsupported)
        ));
        let invalid = app("bad", "bad", "Bad");
        assert!(matches!(
            launch_app(&invalid),
            Err(PlatformError::InvalidComponent(_))
        ));
        assert!(matches!(
            app_icon_rgba("com.example.app", 144),
            Err(PlatformError::Unsupported)
        ));
        assert!(matches!(
            app_icon_rgba("", 144),
            Err(PlatformError::InvalidComponent(_))
        ));
        assert!(matches!(
            system_wallpaper_rgba(1080, 2340),
            Err(PlatformError::Unsupported)
        ));
        assert!(matches!(
            read_system_states(),
            Err(PlatformError::Unsupported)
        ));
        assert!(matches!(
            open_system_screen("wifi"),
            Err(PlatformError::Unsupported)
        ));
    }

    #[test]
    fn system_screen_whitelist() {
        assert_eq!(
            system_screen_action("wifi"),
            Some("android.settings.WIFI_SETTINGS")
        );
        assert_eq!(
            system_screen_action("battery_saver"),
            Some("android.settings.BATTERY_SAVER_SETTINGS")
        );
        for unknown in ["", "flashlight", "arbitrary.action", "wifi " ] {
            assert!(system_screen_action(unknown).is_none(), "{unknown:?} accepted");
        }
        // Unknown keys are rejected before any JNI work happens.
        assert!(matches!(
            open_system_screen("not-a-screen"),
            Err(PlatformError::InvalidComponent(_))
        ));
    }
}

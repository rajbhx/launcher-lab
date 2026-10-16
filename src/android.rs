//! Android entry point (NativeActivity backend, packaged by `cargo apk`).
//!
//! This module is compiled only for `target_os = "android"`. The
//! `android_main` symbol below is the entry point that `android-activity`
//! invokes once the NativeActivity is created.

use winit::platform::android::activity::AndroidApp;

use crate::LauncherLab;

#[no_mangle]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    // Adopt the live JavaVM + activity context so `platform` can talk to the
    // Android system services (PackageManager, WallpaperManager, ...).
    crate::platform::set_android();

    // Hand the app's private data directory to the state store so hidden apps
    // and the recents list survive an OS-initiated process kill.
    crate::store::set_dir(app.internal_data_path());

    // Explicitly request the glow (OpenGL ES) backend with vsync: on Android
    // this drives the NativeActivity's hardware-accelerated EGL surface.
    let options = eframe::NativeOptions {
        android_app: Some(app),
        renderer: eframe::Renderer::Glow,
        vsync: true,
        ..Default::default()
    };

    if let Err(err) = eframe::run_native(
        "Launcher Lab",
        options,
        Box::new(|cc| Ok(Box::new(LauncherLab::new(cc)))),
    ) {
        log::error!("eframe failed to start: {err}");
    }
}

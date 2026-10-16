//! # Launcher Lab
//!
//! A gesture-first launcher prototype written in pure Rust (no Java/Kotlin,
//! no GC) that reconstructs the interaction model REA found in OnePlus
//! Launcher 4.4.8 (`net.oneplus.launcher`). See `docs/ANALYSIS.md` for the
//! static-analysis evidence this build is based on.
//!
//! Rendering uses `egui`/`eframe` (GLES via glow on Android). All input
//! routing flows through a small state machine in [`model::state`] that
//! mirrors the `*InputConsumer` / `TYPE_*` design observed in
//! `net.oneplus.quickstep.TouchInteractionService`.

pub mod model;
pub mod platform;

mod app;
mod clock;
mod icons;
mod theme;
mod views;
mod wallpaper;

#[cfg(target_os = "android")]
mod android;

pub use app::LauncherLab;

/// Desktop entry point (used by `src/main.rs` for local development).
///
/// On Android the app is started through the `android_main` export in
/// `src/android.rs` instead; `cargo apk build` packages that into an APK.
pub fn run_desktop() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Launcher Lab")
            .with_inner_size([412.0, 892.0]),
        renderer: eframe::Renderer::Glow,
        vsync: true,
        ..Default::default()
    };
    eframe::run_native(
        "Launcher Lab",
        options,
        Box::new(|cc| Ok(Box::new(LauncherLab::new(cc)))),
    )
}

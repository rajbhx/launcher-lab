//! Desktop binary. The Android build packages the library (cdylib) instead;
//! this binary exists so the same code can be run on a workstation with
//! `cargo run`.

fn main() -> Result<(), eframe::Error> {
    launcher_lab::run_desktop()
}

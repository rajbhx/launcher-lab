//! Pure-logic model, independent of rendering.
//!
//! These modules mirror structures observed in the analyzed OnePlus Launcher
//! 4.4.8 (input-consumer routing, shelf card contract, Hidden Space), and are
//! covered by unit tests that run on GitHub Actions.

pub mod hidden;
pub mod pager;
pub mod shelf;
pub mod state;

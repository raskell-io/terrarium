//! TUI client for Terrarium simulations.
//!
//! A Dwarf Fortress-inspired terminal viewer with modern keybindings. The UI (app state,
//! drawing, key handling) is shared; the terminal run loop is native-only, and the browser
//! drives the same UI through Ratzilla (web/).

mod app;
mod input;
pub mod theme;
mod ui;
mod widgets;

pub use app::App;
pub use input::{handle_key, Key};
pub use ui::draw;

#[cfg(feature = "native")]
mod terminal;
#[cfg(feature = "native")]
pub use terminal::run;

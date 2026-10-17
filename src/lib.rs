//! Terrarium: a societal simulation engine where agents form emergent civilizations.
//!
//! The engine is a library so that clients can drive it: the native CLI and terminal UI
//! (`native` feature), and the browser build in `web/`, which compiles it to WebAssembly.

pub mod action;
pub mod agent;
pub mod config;
pub mod crafting;
pub mod engine;
pub mod environment;
pub mod groups;
pub mod instinct;
pub mod llm;
pub mod observation;
pub mod observer;
pub mod structures;
pub mod trade;
#[cfg(feature = "tui")]
pub mod tui;
pub mod world;

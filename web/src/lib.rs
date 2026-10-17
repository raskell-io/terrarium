//! Terrarium's terminal viewer in the browser.
//!
//! The same TUI as `terrarium --tui` (its app state, drawing and key handling), rendered into
//! the page by Ratzilla's WebGL2 backend and driven by the engine compiled to WebAssembly. The world comes from
//! the page's `#hash` (a bundled scenario id); W switches world, R starts it over.

use std::cell::RefCell;
use std::future::Future;
use std::pin::pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use ratzilla::event::{KeyCode, KeyEvent};
use ratzilla::ratatui::Terminal;
use ratzilla::backend::webgl2::WebGl2BackendOptions;
use ratzilla::{FontAtlasConfig, WebGl2Backend, WebRenderer};
use terrarium::config::Config;
use terrarium::engine::Engine;
use terrarium::tui::{self, App, Key};
use wasm_bindgen::prelude::*;

/// Scenarios shipped with the page: (id, TOML); the first opens when the address names none
const SCENARIOS: &[(&str, &str)] = &[
    ("great_valley", include_str!("../../scenarios/great_valley.toml")),
    ("first_winter", include_str!("../../scenarios/first_winter.toml")),
    ("exoplanet", include_str!("../../scenarios/exoplanet.toml")),
    ("antarctica", include_str!("../../scenarios/antarctica.toml")),
    ("mars_colony", include_str!("../../scenarios/mars_colony.toml")),
    ("space_station", include_str!("../../scenarios/space_station.toml")),
];

/// Days to run: effectively open-ended, as long as anyone is alive
const OPEN_ENDED_DAYS: usize = 1_000_000;

/// Drive a future to completion. Without an LLM nothing in the engine ever waits
fn run_now<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    match future.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("the browser build has nothing to wait for"),
    }
}

fn now_ms() -> f64 {
    js_sys::Date::now()
}

/// A world being watched: the engine, the TUI's state, and which scenario it is
struct Session {
    scenario: usize,
    engine: Engine,
    app: App,
    last_step: f64,
}

impl Session {
    fn start(scenario: usize, app: App) -> Result<Self, String> {
        let (_, toml) = SCENARIOS[scenario];
        let mut config: Config = ::toml::from_str(toml).map_err(|e| e.to_string())?;
        config.simulation.epochs = OPEN_ENDED_DAYS;
        let mut engine = Engine::in_memory(config).map_err(|e| e.to_string())?;
        engine.initialize().map_err(|e| e.to_string())?;
        Ok(Self { scenario, engine, app, last_step: now_ms() })
    }

    /// Start a (possibly different) world, keeping the viewer's settings
    fn restart(&mut self, scenario: usize) {
        let mut app = std::mem::take(&mut self.app);
        app.selected_agent = None;
        app.selected_index = 0;
        app.events_scroll = 0;
        if let Ok(next) = Session::start(scenario, app) {
            *self = next;
            set_hash(SCENARIOS[scenario].0);
        }
    }

    fn step(&mut self) {
        if self.engine.alive_count() > 0 {
            let _ = run_now(self.engine.step());
        }
    }

    fn on_key(&mut self, event: KeyEvent) {
        let key = match event.code {
            KeyCode::Char(c) => Key::Char(c),
            KeyCode::Tab if event.shift => Key::BackTab,
            KeyCode::Tab => Key::Tab,
            KeyCode::Esc => Key::Esc,
            KeyCode::Left => Key::Left,
            KeyCode::Right => Key::Right,
            KeyCode::Up => Key::Up,
            KeyCode::Down => Key::Down,
            KeyCode::PageUp => Key::PageUp,
            KeyCode::PageDown => Key::PageDown,
            _ => Key::Other,
        };
        if !self.app.show_help {
            match key {
                Key::Char('w') | Key::Char('W') => return self.restart((self.scenario + 1) % SCENARIOS.len()),
                Key::Char('r') | Key::Char('R') => return self.restart(self.scenario),
                Key::Char('n') | Key::Char('N') if !self.app.running => self.step(),
                _ => {}
            }
        }
        tui::handle_key(key, &mut self.app, &self.engine);
    }

    /// Called every animation frame: advance at the TUI's speed, then draw
    fn frame(&mut self, frame: &mut ratzilla::ratatui::Frame) {
        let now = now_ms();
        if self.app.running && now - self.last_step >= self.app.speed_ms as f64 {
            self.step();
            self.last_step = now;
        }
        tui::draw(frame, &self.engine, &mut self.app);
    }
}

fn hash_scenario() -> usize {
    let hash = web_sys::window().and_then(|w| w.location().hash().ok()).unwrap_or_default();
    let id = hash.trim_start_matches('#');
    SCENARIOS.iter().position(|(s, _)| *s == id).unwrap_or(0)
}

fn set_hash(id: &str) {
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_hash(id);
    }
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    // Report Rust panics with their message in the browser console
    console_error_panic_hook::set_once();
    let mut app = App::new();
    // A web page should come alive on its own; Space pauses
    app.running = true;
    app.can_quit = false;
    app.client_keys = vec![("W", "Next world"), ("R", "Start this world over")];
    let session = Session::start(hash_scenario(), app).map_err(|e| JsValue::from_str(&e))?;
    let session = Rc::new(RefCell::new(session));

    // WebGL2 resizes cleanly with the window (the DOM backend panics when its grid shrinks)
    // and draws the page's own font
    let options = WebGl2BackendOptions::new()
        .font_atlas_config(FontAtlasConfig::dynamic(&["JetBrains Mono"], 16.0))
        .canvas_padding_color(terrarium::tui::theme::BG);
    let backend = WebGl2Backend::new_with_options(options).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let mut terminal = Terminal::new(backend).map_err(|e| JsValue::from_str(&e.to_string()))?;

    terminal
        .on_key_event({
            let session = session.clone();
            move |event| session.borrow_mut().on_key(event)
        })
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    terminal.draw_web(move |frame| session.borrow_mut().frame(frame));
    Ok(())
}

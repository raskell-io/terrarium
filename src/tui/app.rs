//! Application state for the TUI.

use uuid::Uuid;

/// TUI application state
pub struct App {
    /// Whether simulation is running (auto-advancing)
    pub running: bool,

    /// Speed in milliseconds per epoch
    pub speed_ms: u32,

    /// Currently selected agent ID
    pub selected_agent: Option<Uuid>,

    /// Selected agent index (for cycling)
    pub selected_index: usize,

    /// Events scroll offset
    pub events_scroll: usize,

    /// Show help overlay
    pub show_help: bool,

    /// Show full agent details
    pub show_full_agent: bool,

    /// Show events panel
    pub show_events: bool,

    /// Show agent panel
    pub show_agent: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            running: false,
            speed_ms: 500,
            selected_agent: None,
            selected_index: 0,
            events_scroll: 0,
            show_help: false,
            show_full_agent: false,
            show_events: true,
            show_agent: true,
        }
    }

    /// Toggle pause/play
    pub fn toggle_running(&mut self) {
        self.running = !self.running;
    }

    /// Increase speed (decrease interval)
    pub fn speed_up(&mut self) {
        self.speed_ms = (self.speed_ms / 2).max(50);
    }

    /// Decrease speed (increase interval)
    pub fn slow_down(&mut self) {
        self.speed_ms = (self.speed_ms * 2).min(2000);
    }

    /// Select next agent
    pub fn select_next(&mut self, agents: &[uuid::Uuid]) {
        if agents.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + 1) % agents.len();
        self.selected_agent = Some(agents[self.selected_index]);
    }

    /// Select previous agent
    pub fn select_prev(&mut self, agents: &[uuid::Uuid]) {
        if agents.is_empty() {
            return;
        }
        if self.selected_index == 0 {
            self.selected_index = agents.len() - 1;
        } else {
            self.selected_index -= 1;
        }
        self.selected_agent = Some(agents[self.selected_index]);
    }

    /// Select agent by index (1-9)
    pub fn select_by_number(&mut self, n: usize, agents: &[uuid::Uuid]) {
        let idx = n.saturating_sub(1);
        if idx < agents.len() {
            self.selected_index = idx;
            self.selected_agent = Some(agents[idx]);
        }
    }

    /// Ensure a valid agent is selected
    pub fn ensure_selection(&mut self, agents: &[uuid::Uuid]) {
        if agents.is_empty() {
            self.selected_agent = None;
            return;
        }

        // Check if current selection is still valid
        if let Some(id) = self.selected_agent {
            if !agents.contains(&id) {
                // Current selection died, select another
                self.selected_index = self.selected_index.min(agents.len() - 1);
                self.selected_agent = Some(agents[self.selected_index]);
            }
        } else {
            // No selection, pick first
            self.selected_index = 0;
            self.selected_agent = Some(agents[0]);
        }
    }

    /// Scroll events up
    pub fn scroll_events_up(&mut self) {
        self.events_scroll = self.events_scroll.saturating_add(3);
    }

    /// Scroll events down
    pub fn scroll_events_down(&mut self) {
        self.events_scroll = self.events_scroll.saturating_sub(3);
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

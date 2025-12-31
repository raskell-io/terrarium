# TUI Client

A terminal-based viewer for Terrarium simulations, inspired by Dwarf Fortress but with modern keybindings.

## Layout

```
┌─ World ─────────────────────┐┌─ Events ──────────────────────┐
│ . . . * . . B . . *         ││ Day 47                         │
│ . A . . . . . * . .         ││ ► Bria gave 2 food to Corin    │
│ . . . . . . . . . .         ││   Danen attacked Elwyn!        │
│ * . . . . . . C . *         ││   Elwyn fled north             │
│ . . . . D . . . . .         ││                                │
│ . . . . . . . . . .         ││ Day 46                         │
│ . * . . . . . . * .         ││   Faya: "Stay away from Danen" │
│ . . E . . . . . . .         ││   Garen gathered 5 food        │
│ * . . . . . F . . .         ││                                │
│ . . . . . . . . * .         ││                                │
└─────────────────────────────┘└────────────────────────────────┘
┌─ Bria ───────────────────────────────────────────────────────┐
│ Health ████████░░ 80%   Hunger ██░░░░░░░░ 20%   Food: 5      │
│ Energy ██████░░░░ 60%   Position (6,0)   Goal: Explore       │
│                                                              │
│ curious and creative, cooperative and trusting, anxious      │
│ Aspiration: to protect those around me                       │
│                                                              │
│ Relationships:                                               │
│   Corin: ♥♥♥♥♡ trusts    Danen: ♥♡♡♡♡ distrusts            │
│                                                              │
│ Recent: "I gave food to Corin" (Day 47)                      │
└──────────────────────────────────────────────────────────────┘
─────────────────────────────────────────────────────────────────
 Space: Pause/Play │ N: Step │ ←↑↓→: Select │ Q: Quit │ ?: Help
```

## Panels

### World Panel (top-left)
- ASCII grid representation
- `.` = barren terrain
- `*` = fertile terrain (has food)
- `A-Z` = agents (letter from name)
- Selected agent highlighted

### Events Panel (top-right)
- Scrolling log of recent events
- Grouped by day
- Most recent at top
- Speech shown in quotes

### Agent Panel (bottom)
- Details of selected agent
- Health/hunger/energy as progress bars
- Position and current goal
- Personality summary
- Relationships with trust/sentiment indicators
- Recent memories

### Status Bar (bottom)
- Available keybindings
- Current simulation state (running/paused)
- Current epoch

## Keybindings

### Navigation
| Key | Action |
|-----|--------|
| `↑` `↓` `←` `→` | Select adjacent agent on map |
| `Tab` | Cycle to next agent |
| `Shift+Tab` | Cycle to previous agent |
| `1-9` | Jump to agent by number |

### Simulation Control
| Key | Action |
|-----|--------|
| `Space` | Pause / Resume |
| `n` | Step one epoch (when paused) |
| `+` / `=` | Increase speed |
| `-` | Decrease speed |
| `r` | Restart simulation |

### View
| Key | Action |
|-----|--------|
| `e` | Toggle events panel |
| `a` | Toggle agent panel |
| `f` | Toggle full agent details |
| `m` | Center map on selected agent |
| `Page Up` | Scroll events up |
| `Page Down` | Scroll events down |

### General
| Key | Action |
|-----|--------|
| `q` | Quit |
| `?` | Show help |
| `Esc` | Close help / Cancel |

## Visual Elements

### Agent Display on Map
- First letter of name (A for Aric, B for Bria, etc.)
- Selected agent shown in highlight/inverse
- Dead agents shown as `†`

### Health/Hunger/Energy Bars
```
████████░░ 80%    (green if >60%, yellow if >30%, red if ≤30%)
```

### Trust/Sentiment Indicators
```
Trust:     ♥♥♥♥♡ (+0.8)   or   ♡♡♡♡♡ (-0.8)
Sentiment: likes           or   dislikes
```

### Event Icons
```
► Movement
◆ Gathering
♦ Eating/Resting
💬 Speech (or just quotes)
🎁 Gift (or →)
⚔ Attack (or !)
💀 Death (or †)
```

## Colors

| Element | Color |
|---------|-------|
| Fertile terrain | Green |
| Barren terrain | Dark gray |
| Selected agent | Yellow/highlight |
| Other agents | White |
| Dead agents | Dark red |
| Health bar (healthy) | Green |
| Health bar (injured) | Yellow |
| Health bar (critical) | Red |
| Positive sentiment | Green |
| Negative sentiment | Red |
| Events | Cyan |
| Speech | Yellow |

## Modes

### Running Mode
- Simulation advances automatically
- Speed controlled by +/- keys
- Events stream in real-time

### Paused Mode
- Simulation frozen
- Can step one epoch at a time with `n`
- Can inspect agents at leisure

### Help Mode
- Overlay showing all keybindings
- Press `?` to toggle
- Press `Esc` or `?` to close

## Implementation

### Dependencies
- `ratatui` - Terminal UI framework
- `crossterm` - Cross-platform terminal handling

### Architecture
```
tui/
├── mod.rs           # Public interface
├── app.rs           # Application state
├── ui.rs            # Rendering logic
├── widgets/
│   ├── world.rs     # Map widget
│   ├── events.rs    # Event log widget
│   └── agent.rs     # Agent panel widget
└── input.rs         # Key handling
```

### Integration with Engine

The TUI embeds the engine directly:

```rust
// Create engine
let engine = Engine::new(config)?;

// TUI controls simulation
loop {
    // Handle input
    if key == 'n' && paused {
        engine.step().await?;
    }

    // Get current state
    let world_view = engine.world_view();
    let agent_views = engine.agent_views();
    let events = engine.recent_events();

    // Render
    terminal.draw(|f| {
        render_world(f, &world_view);
        render_events(f, &events);
        render_agent(f, &selected_agent);
    })?;
}
```

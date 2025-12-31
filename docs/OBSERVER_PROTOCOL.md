# Observer Protocol

Terrarium separates the **simulation engine** from **observation clients**. This document defines the interface between them.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     SIMULATION ENGINE                           │
│                                                                 │
│  ┌─────────┐    ┌─────────┐    ┌─────────┐                     │
│  │  World  │    │ Agents  │    │ Actions │                     │
│  └─────────┘    └─────────┘    └─────────┘                     │
│                                                                 │
└──────────────────────────┬──────────────────────────────────────┘
                           │
                           │ Observer Protocol
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│                    OBSERVER INTERFACE                           │
│                                                                 │
│  - World state (grid, terrain, resources)                       │
│  - Agent states (position, health, beliefs, memory)             │
│  - Event stream (what happened this epoch)                      │
│  - Simulation control (pause, step, resume, speed)              │
│                                                                 │
└──────────────────────────┬──────────────────────────────────────┘
                           │
           ┌───────────────┼───────────────┐
           │               │               │
           ▼               ▼               ▼
      ┌─────────┐    ┌─────────┐    ┌─────────┐
      │   TUI   │    │ Web UI  │    │  Query  │
      │ Client  │    │ Client  │    │   CLI   │
      └─────────┘    └─────────┘    └─────────┘
```

## Observer Interface

Any client can implement the observer interface to visualize or analyze the simulation.

### Data Available

#### World State
```rust
struct WorldView {
    epoch: usize,
    width: usize,
    height: usize,
    cells: Vec<CellView>,
}

struct CellView {
    x: usize,
    y: usize,
    terrain: Terrain,      // Fertile or Barren
    food: u32,
    occupants: Vec<Uuid>,  // Agent IDs at this cell
}
```

#### Agent State
```rust
struct AgentView {
    id: Uuid,
    name: String,
    position: (usize, usize),

    // Physical
    health: f64,           // 0.0 - 1.0
    hunger: f64,           // 0.0 - 1.0
    energy: f64,           // 0.0 - 1.0
    food: u32,
    alive: bool,

    // Identity
    personality_summary: String,
    aspiration: String,

    // Cognitive
    current_goal: Option<String>,
    recent_memories: Vec<String>,
    social_beliefs: Vec<SocialBeliefView>,
}

struct SocialBeliefView {
    about: String,         // Agent name
    trust: f64,            // -1.0 to 1.0
    sentiment: f64,        // -1.0 to 1.0
}
```

#### Events
```rust
enum EventView {
    EpochStart { epoch: usize },
    EpochEnd { epoch: usize },
    Moved { agent: String, from: (usize, usize), to: (usize, usize) },
    Gathered { agent: String, amount: u32 },
    Ate { agent: String },
    Rested { agent: String },
    Spoke { agent: String, target: String, message: String },
    Gave { agent: String, target: String, amount: u32 },
    Attacked { agent: String, target: String, damage: f64 },
    Died { agent: String, cause: String },
}
```

### Control Commands

```rust
enum SimulationCommand {
    Pause,
    Resume,
    Step,                  // Advance one epoch
    SetSpeed(f64),         // Epochs per second (when running)
    Stop,                  // End simulation
}
```

## Client Modes

### 1. Live Mode

Client connects to running simulation:
- Receives state updates after each epoch
- Can send control commands
- Real-time observation

### 2. Replay Mode

Client reads from completed simulation files:
- Load events.jsonl and states/*.json
- Replay at any speed
- Seek to any epoch
- No control commands (simulation already complete)

### 3. Embedded Mode

Client embeds the engine as a library:
- Direct function calls
- No serialization overhead
- Full control

## File Formats

For persistence and replay, the simulation writes:

### events.jsonl
One JSON object per line, each event as it occurs.
```json
{"epoch":0,"event_type":"EpochStart","agent":null,"target":null,"data":{}}
{"epoch":0,"event_type":"Moved","agent":"uuid","target":null,"data":{"from":[0,0],"to":[1,0]}}
```

### states/epoch_NNNN.json
Full state snapshot at specific epochs.
```json
{
  "epoch": 50,
  "world": { ... },
  "agents": [ ... ]
}
```

## Implementation Notes

### TUI Client

The TUI client operates in **embedded mode**:
- Runs the engine directly
- Calls `engine.step()` to advance
- Reads state via `engine.world_view()` and `engine.agent_views()`
- Renders to terminal using ratatui

### Future Web Client

Could operate in **replay mode** or via **WebSocket for live mode**:
- Load JSON files via HTTP
- Or connect to a simulation server
- Render using browser technologies

### Query CLI

Operates in **replay mode**:
- Loads completed simulation data
- Answers questions about history
- No visualization, just text output

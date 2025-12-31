# Architecture

## Overview

Terrarium has three main components:

```
┌─────────────────────────────────────────────────────────┐
│                     OBSERVER                            │
│            (You, reading logs and analysis)             │
└─────────────────────────────────────────────────────────┘
                          │
                          │ reads
                          ▼
┌─────────────────────────────────────────────────────────┐
│                   SIMULATION                            │
│                                                         │
│   ┌─────────────┐              ┌─────────────────────┐ │
│   │   WORLD     │◄────────────►│      AGENTS         │ │
│   │   ENGINE    │  constrains/ │                     │ │
│   │             │  perceives   │  ┌───┐ ┌───┐ ┌───┐  │ │
│   │ - Grid      │              │  │ A │ │ B │ │...│  │ │
│   │ - Resources │              │  └───┘ └───┘ └───┘  │ │
│   │ - Rules     │              │                     │ │
│   └─────────────┘              └─────────────────────┘ │
│                                         │              │
│                                         │ decides      │
│                                         ▼              │
│                                 ┌───────────────┐      │
│                                 │      LLM      │      │
│                                 │    (Claude)   │      │
│                                 └───────────────┘      │
│                                                        │
└─────────────────────────────────────────────────────────┘
                          │
                          │ writes
                          ▼
┌─────────────────────────────────────────────────────────┐
│                   EVENT LOG                             │
│            (events.jsonl + snapshots)                   │
└─────────────────────────────────────────────────────────┘
```

## World Engine

The world defines the physical reality agents exist in.

### Space

- **Grid**: 2D array of cells
- **Cell**: Has terrain type, resources, and occupants
- **Terrain types**: Fertile (produces food), Barren (empty)
- **Visibility**: Agents see their cell and adjacent cells

### Resources

- **Food**: Exists in fertile cells, consumed by eating
- **Regeneration**: 10% of max capacity per epoch
- **Depletion**: Gathering removes food from cell

### Physical Laws

- **Movement**: 1 cell per epoch (8 directions)
- **Hunger**: Increases each epoch, damages health if too high
- **Energy**: Depleted by actions, recovered by resting
- **Health**: Damaged by hunger/attacks, death at zero

## Agent Cognitive Model

Each agent has a layered cognitive architecture:

### Layer 1: Identity (Stable)

```
Identity
├── Personality (Big Five traits, 0.0-1.0)
│   ├── Openness
│   ├── Conscientiousness
│   ├── Extraversion
│   ├── Agreeableness
│   └── Neuroticism
├── Values (what matters most)
│   └── e.g., Survival, Status, Relationships, Freedom
└── Aspiration (life goal)
    └── e.g., "become respected", "protect others", "accumulate resources"
```

Identity is set at agent creation and doesn't change.

### Layer 2: Beliefs (Updateable)

```
Beliefs
├── World beliefs
│   └── "Cell (3,4) has food" — may be outdated
├── Social beliefs
│   └── "Bria is trustworthy" — may be wrong
├── Self beliefs
│   └── "I am strong" — may be deluded
└── Causal beliefs
    └── "Sharing leads to reciprocity" — may be naive
```

Beliefs update from experience. They can be incorrect.

### Layer 3: Working State (Per-Tick)

```
Working State
├── Perception (what's visible now)
├── Physical state (hunger, energy, health, position)
├── Recent events (last 5 epochs)
├── Active goal (current objective)
└── Nearby agents (who's around)
```

This is what the agent "sees" each tick.

### Layer 4: Action Selection

The LLM receives: Identity + Beliefs + Working State

The LLM returns: Reasoning + Chosen Action

Actions are low-level:
- `move(direction)`
- `gather`
- `eat`
- `rest`
- `speak(target, message)`
- `give(target, amount)`
- `attack(target)`

## Simulation Loop

```
for epoch in 0..max_epochs:

    1. WORLD_TICK
       - Regenerate resources
       - Increment agent hunger
       - Apply ongoing effects

    2. PERCEPTION (parallel)
       for each agent:
           - Read visible cells
           - Note nearby agents
           - Build working state

    3. DELIBERATION (parallel, async)
       for each agent:
           - Build LLM prompt (identity + beliefs + working state)
           - Call LLM
           - Parse action from response
           - Log reasoning (optional)

    4. RESOLUTION (simultaneous)
       - Collect all actions
       - Validate against rules
       - Resolve conflicts:
           - Multiple gather → split resources
           - Mutual attacks → both take damage
       - Apply state changes

    5. BELIEF_UPDATES
       for each agent:
           - Record significant events to memory
           - Update social beliefs based on interactions
           - Consolidate old memories into beliefs

    6. LOGGING
       - Append events to events.jsonl
       - Save state snapshot (if epoch % interval == 0)
```

## Event Sourcing

All state changes are recorded as immutable events:

```json
{"epoch": 47, "type": "moved", "agent": "uuid", "from": [2,3], "to": [2,4]}
{"epoch": 47, "type": "gathered", "agent": "uuid", "cell": [2,4], "amount": 5}
{"epoch": 47, "type": "spoke", "agent": "uuid", "target": "uuid2", "content": "I need food"}
{"epoch": 47, "type": "attacked", "agent": "uuid", "target": "uuid2", "damage": 0.2}
```

This allows:
- Replay any epoch
- Query historical patterns
- Reconstruct state at any point

## Conflict Resolution

Simultaneous resolution means actions are decided on frozen state, then resolved together.

| Conflict | Resolution |
|----------|------------|
| Multiple gather same cell | Split resources by strength (or equal) |
| Mutual attack | Both take damage proportional to other's strength |
| Move to occupied cell | Allowed (multiple agents can share cell) |
| Give to non-adjacent | Action fails silently |
| Attack non-adjacent | Action fails silently |

## LLM Integration

### Prompt Structure

```
SYSTEM: You are {name}, a person living in a small world...
[Personality description]
[Values and aspiration]
[World constraints and rules]

USER:
## Current State
{physical state: hunger, energy, health, position}

## What You Believe
{world beliefs}
{social beliefs}

## What You See
{current cell description}
{nearby cells}
{nearby agents}

## Recent Events
{last 5 epochs summary}

## Your Current Goal
{active goal}

## Available Actions
{list of valid actions}

What do you do? Think step by step, then choose ONE action.
```

### Response Parsing

Extract action from natural language response:
- Look for action keywords (MOVE, GATHER, SPEAK, etc.)
- Parse parameters (direction, target, message)
- Validate against rules
- Fall back to WAIT if unparseable

## Observation Layer

### Event Log (`events.jsonl`)

Append-only log of all events. Machine-readable.

### State Snapshots (`states/epoch_N.json`)

Full world and agent state at specific epochs. Allows reconstruction.

### Chronicle (`chronicle.md`)

Human-readable narrative generated from events:

> **Epoch 47, Spring**
>
> Tensions rose at the eastern berry patch. Danen, hungry and desperate,
> confronted Elwyn who had been gathering there for days. Words were exchanged.
> Danen struck. Elwyn fled north, injured.
>
> Later that day, Faya warned Goren: "Stay away from Danen."

Chronicle is for human reading. Events log is for analysis.

## Technology Choices

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Language | Rust | Performance, type safety, concurrency |
| LLM | Claude API | Quality, reliability, structured output |
| Storage | JSON files | Simple, portable, no dependencies |
| Config | TOML | Human-readable, standard in Rust ecosystem |

## Future Considerations

### Scaling

- **Selective LLM calls**: Only call for "interesting" decisions
- **Behavioral caching**: Similar situation → similar action
- **Tiered models**: Routine=cheap, complex=expensive
- **Local models**: For bulk processing

### Richer World

- Multiple resource types
- Crafting/transformation
- Seasonal effects
- Environmental events

### Better Observation

- Metrics dashboard
- Anomaly detection
- Query interface
- Visualization

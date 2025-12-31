# Terrarium - Development Guide

> A societal simulation engine where LLM-powered agents form emergent civilizations.

## Project Status

**Phase**: MVP Development (Tribe Scale)

## Core Design Decisions

These have been resolved through design discussion:

| Decision | Resolution |
|----------|------------|
| Agent knowledge | Beliefs, not facts. Agents can be wrong. |
| Time granularity | Daily epochs, simultaneous resolution |
| Property | Belief only. No enforcement mechanic. Violence is the arbiter. |
| Observation | Event sourcing, post-hoc analysis |
| Emergence detection | Metrics + anomaly flags + human investigation |
| Scale | Start tribe (10), validate model, then grow |
| Validity | LLMs shaped via system prompts + needs hierarchy |

## Cognitive Architecture

Agents are not simple prompt→response. They have layered cognition:

```
IDENTITY (stable)
├── Personality traits (Big Five simplified)
├── Core values
└── Life aspirations
        │
        ▼
BELIEF SYSTEM (updates from experience)
├── World beliefs ("the forest has food") — can be wrong
├── Social beliefs ("Bria is trustworthy") — can be wrong
├── Self beliefs ("I am a good hunter") — can be deluded
└── Causal beliefs ("sharing leads to reciprocity") — can be naive
        │
        ▼
WORKING STATE (current tick)
├── Perception (what's visible now)
├── Active goal (what I'm trying to do)
├── Recent events (last few ticks)
└── Physical state (hunger, energy, health)
        │
        ▼
ACTION (low-level)
└── move, gather, eat, speak, give, attack, rest
```

## World Engine Layers

```
PHYSICAL (hard rules)
├── Space: 10x10 grid
├── Terrain: Fertile (food) or Barren
├── Bodies: hunger, energy, health, position
└── Resources: food regenerates slowly

ECONOMIC (enabled by physical)
├── Possession: agents carry inventory
├── No trade mechanic: use give + trust
└── No property mechanic: belief only

SOCIAL (enabled by communication)
├── Speech: requires proximity
├── Reputation: beliefs about others, spread via gossip
└── Groups: not programmed, must emerge
```

## Simulation Loop

```
for each epoch:
    1. WORLD TICK
       - Resources regenerate
       - Hunger/energy update

    2. PERCEPTION (parallel)
       - Each agent sees local environment
       - Updates working state

    3. DELIBERATION (parallel, LLM calls)
       - Agent receives: identity + beliefs + working state
       - LLM returns: reasoning + action

    4. RESOLUTION (simultaneous)
       - All actions validated
       - Conflicts resolved (gather splits, attacks mutual)
       - State changes applied

    5. BELIEF UPDATES
       - Agents update beliefs based on outcomes
       - Memory consolidation

    6. LOGGING
       - Events → events.jsonl
       - State snapshot (periodic)
```

## MVP Scope

**Goal**: Validate that the model produces interesting, coherent dynamics.

**Agents**: 10
- Full cognitive model
- Simplified Big Five personality
- Basic needs: hunger, energy
- Memory: recent events + beliefs about others
- One aspiration per agent

**World**: 10x10 grid
- 2 terrain types only
- Food regenerates 10%/epoch
- No crafting, no materials

**Actions**: Minimal
- move, gather, eat, rest
- speak, give, attack

**Duration**: 100 epochs

**Output**: Logs only (no UI)

**Scenario**: "First Winter" - scarcity pressure without instant death

## File Structure

```
terrarium/
├── src/
│   ├── main.rs           # CLI entry
│   ├── engine.rs         # Simulation loop
│   ├── world.rs          # Grid, terrain, resources
│   ├── agent/
│   │   ├── mod.rs        # Agent struct
│   │   ├── identity.rs   # Personality, values, aspirations
│   │   ├── beliefs.rs    # World/social/self beliefs
│   │   └── memory.rs     # Episode storage, consolidation
│   ├── action.rs         # Action types and resolution
│   ├── llm.rs            # LLM integration
│   └── observation/
│       ├── events.rs     # Event types
│       └── chronicle.rs  # Narrative generation
├── scenarios/
│   └── first_winter.toml
├── output/               # Generated
│   ├── events.jsonl
│   ├── states/
│   └── chronicle.md
└── docs/
    ├── CONCEPT.md        # Vision and philosophy
    ├── ARCHITECTURE.md   # Technical decisions
    └── MVP.md            # Current scope
```

## Key Implementation Notes

### LLM Prompting

Each agent prompt includes:
1. Identity block (personality, values, aspiration)
2. Current beliefs (what I think I know)
3. Working state (what I see, what I need, what I'm doing)
4. Available actions
5. Request for reasoning + action choice

### Belief Updates

Beliefs consolidate from episodes:
```
Episodes:
  - "Bria gave me food" (epoch 10)
  - "Bria traded fairly" (epoch 25)
  - "Bria warned me of danger" (epoch 40)
          ↓
Belief: { agent: "Bria", trust: 0.7, pattern: "helpful" }
```

Raw episodes can fade. Consolidated beliefs persist.

### Conflict Resolution

Simultaneous resolution rules:
- **Gather same cell**: Split by relative strength (or equal)
- **Mutual attack**: Both take damage proportional to other's strength
- **Move to same cell**: Both succeed (cell can have multiple occupants)

### Event Types

```
PHYSICAL:   moved, gathered, consumed, health_changed, died
SOCIAL:     spoke, relationship_changed
ECONOMIC:   gave
CONFLICT:   attacked, fled
INTERNAL:   goal_changed, belief_updated (if logging thoughts)
```

## Success Criteria

**Minimum**:
- Agents survive (some) for 100 epochs
- Differentiated beliefs form
- Personality affects behavior
- One observable emergent dynamic

**Strong**:
- Recognizable social structure
- Resource-sharing patterns
- Traceable conflict causes
- Chronicle is interesting to read

## Commands

```bash
# Build
cargo build --release

# Run MVP scenario
./target/release/terrarium --scenario scenarios/first_winter.toml

# Run with debug logging
RUST_LOG=debug ./target/release/terrarium --scenario scenarios/first_winter.toml
```

## Tech Stack

- **Language**: Rust (performance, type safety)
- **LLM**: Claude API (provider-agnostic interface)
- **Storage**: JSON files for MVP (event sourcing)
- **Config**: TOML

## Open Questions (for future phases)

- Memory compression at scale
- Hierarchical agent abstraction for large populations
- Automated emergence detection
- Birth/death/reproduction mechanics

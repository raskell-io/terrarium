# Terrarium

A societal simulation engine where LLM-powered agents form emergent civilizations.

## What is this?

Terrarium is a **societal petri dish**. You create a world with rules, populate it with autonomous agents (each powered by an LLM), and observe what emerges.

Each agent has:
- **Personality**: Stable traits that bias behavior
- **Beliefs**: Updateable, possibly wrong, shaped by experience
- **Needs**: Hunger, energy—survival pressure
- **Memory**: What happened to them, what they think of others

You don't program markets, hierarchies, or alliances. You watch for them.

## The Core Question

> Can we observe the organic emergence of social structures by giving agents freedom within constraints?

## Status

**Phase**: MVP Design Complete

Currently: Conceptual model validated. Ready for implementation.

## Documentation

| Document | Description |
|----------|-------------|
| [Concept](docs/CONCEPT.md) | Vision, philosophy, inspirations |
| [Architecture](docs/ARCHITECTURE.md) | Technical design, cognitive model, simulation loop |
| [MVP](docs/MVP.md) | Current scope: 10 agents, 100 epochs, "First Winter" scenario |

## MVP Scope

- **10 agents** with full cognitive model
- **10x10 grid** with fertile/barren terrain
- **7 actions**: move, gather, eat, rest, speak, give, attack
- **100 epochs** of simulation
- **Output**: Event logs + human-readable chronicle

Goal: Validate that the model produces interesting, coherent dynamics.

## Key Design Decisions

| Decision | Choice |
|----------|--------|
| Agent knowledge | Beliefs, not facts (agents can be wrong) |
| Property | Belief only, no enforcement (violence is the arbiter) |
| Time | Daily epochs, simultaneous resolution |
| Observation | Event sourcing, post-hoc analysis |
| Scale | Start with tribe (10), validate, then grow |

## Inspirations

- *Dwarf Fortress* — emergent narratives from simple systems
- *Conway's Game of Life* — complexity from minimal rules
- *Guns, Germs, and Steel* — geography shaping civilization
- Agent-based modeling in computational social science

## Quick Start

```bash
cargo build --release
./target/release/terrarium --scenario scenarios/first_winter.toml
cat output/chronicle.md
```

## What Might Emerge?

We don't know. That's the point. Possibilities:
- Trade networks (reciprocal exchange)
- Property norms (respected claims)
- Hierarchies (asymmetric deference)
- Alliances (coordinated action)
- Reputations (beliefs about others spreading)
- Conflict patterns (scarcity-driven or personality-driven)

## License

MIT

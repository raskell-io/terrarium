# Terrarium

A societal simulation engine where LLM-powered agents form emergent civilizations.

## What is this?

Terrarium is a **societal petri dish**. Set initial conditions, define the rules of your world, seed it with autonomous agents, then observe what emerges. Trade networks. Social hierarchies. Conflicts. Cooperation. Cultures.

Each agent is powered by an LLM, giving them genuine decision-making capability within the constraints of their personality, memories, and circumstances.

## Why?

To ask questions like:
- Does trade emerge organically when agents have complementary needs?
- How do leadership structures form without explicit hierarchies?
- What conditions lead to cooperation vs. conflict?
- Can we observe the "invisible hand" in action?

This isn't a game to win. It's a **thought experiment generator**.

## Quick Start

```bash
cargo build --release
./target/release/terrarium --agents 10 --epochs 100
cat output/chronicle.md
```

## Scale

| Tier | Agents | What emerges |
|------|--------|--------------|
| Tribe | ~10 | Personal relationships, leadership |
| Village | ~100 | Specialization, customs, early trade |
| Town | ~1,000 | Markets, governance, institutions |
| City | ~10,000 | Classes, bureaucracy, culture |
| Society | ~100,000 | States, economies, ideologies |

## Inspired by

- Conway's Game of Life
- Dwarf Fortress
- Anno / The Settlers
- Jared Diamond's "Guns, Germs, and Steel"
- Agent-based modeling in social sciences

## Status

Early development. Currently building the core simulation loop.

## License

MIT

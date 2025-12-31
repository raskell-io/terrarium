# MVP: First Winter

## Goal

Validate that the core model works: LLM-powered agents with personality and beliefs, operating in a constrained world, produce emergent social dynamics worth observing.

## Scope

### Agents: 10

Each agent has:

**Identity (fixed at creation)**
- Name
- Personality: 5 traits (simplified Big Five), each 0.0-1.0
- Values: 2-3 priorities (e.g., Survival, Relationships, Status)
- Aspiration: One life goal (e.g., "be respected", "help others", "accumulate resources")

**Beliefs (update from experience)**
- World beliefs: What's where, what's safe, what's dangerous
- Social beliefs: Trust/distrust, like/dislike for each known agent
- Self beliefs: What am I good at, what do I want

**Physical state**
- Position: (x, y) on grid
- Hunger: 0.0 (full) to 1.0 (starving)
- Energy: 0.0 (exhausted) to 1.0 (rested)
- Health: 0.0 (dead) to 1.0 (healthy)
- Inventory: Amount of food carried

**Memory**
- Recent events: Last 5 epochs
- Consolidated beliefs about other agents

### World: 10x10 Grid

**Terrain**
- ~30% Fertile cells (produce food)
- ~70% Barren cells (empty)
- No impassable terrain

**Resources**
- Food only (no materials, no crafting)
- Fertile cells hold 0-20 food
- Regeneration: 10% of capacity per epoch
- Gathering: Remove up to 5 food per action

**Rules**
- Movement: 8 directions, 1 cell per epoch
- Visibility: Current cell + 8 adjacent cells
- Co-location: Multiple agents can occupy same cell

### Actions: 7 Types

| Action | Effect |
|--------|--------|
| `move(direction)` | Change position by 1 cell |
| `gather` | Take food from current cell (up to 5) |
| `eat` | Consume 1 food, reduce hunger by 0.3 |
| `rest` | Recover 0.3 energy |
| `speak(target, message)` | Communicate (target must be nearby) |
| `give(target, amount)` | Transfer food (target must be nearby) |
| `attack(target)` | Deal damage (target must be nearby) |

### Duration: 100 Epochs

Long enough to see patterns form (or not).

### Output: Logs Only

- `events.jsonl`: All events, machine-readable
- `states/`: Periodic snapshots
- `chronicle.md`: Human-readable narrative

No UI, no dashboard, no visualization. Manual inspection only.

## The Scenario: First Winter

**Setup**
- 10 agents with randomized personalities
- Scattered across the grid
- Each starts with 10 food
- World has enough food for ~50 agent-epochs of consumption
- Must gather to survive

**Pressure**
- Not enough food for everyone to survive passively
- Agents must gather actively
- Competition for fertile cells likely
- Cooperation possible but not guaranteed

**What we're looking for**
- Do agents cluster around fertile areas?
- Do patterns of avoidance or confrontation emerge?
- Do any agents share? Under what conditions?
- Do "reputations" form? (X is dangerous, Y is helpful)
- Do any agents die? From starvation or violence?

## Success Criteria

### Minimum (validates the model works)

- [ ] At least some agents survive 100 epochs
- [ ] Agents form differentiated beliefs about each other
- [ ] Behavior patterns differ based on personality
- [ ] At least one interesting dynamic is observable

### Strong (model produces rich behavior)

- [ ] Recognizable social structure (pairs, groups, outcasts)
- [ ] Resource-sharing or reciprocity patterns
- [ ] Conflicts have traceable causes (personality + circumstances)
- [ ] Reading the chronicle is genuinely interesting

### Failure Signals

- All agents behave identically despite different personalities
- Beliefs don't update meaningfully
- Pure chaos with no patterns
- Agents constantly "break character"
- Nothing interesting happens

## What's Explicitly Deferred

| Feature | Why not now |
|---------|-------------|
| Multiple resource types | Unnecessary complexity |
| Crafting | Unnecessary complexity |
| Terrain variety | Unnecessary complexity |
| Trade mechanic | Can emerge from give + trust |
| Property mechanic | Should emerge from belief + defense |
| Seasons | Adds variance, not needed to validate |
| Birth/death (reproduction) | Later phase |
| UI/visualization | Logs are sufficient |
| Metrics dashboard | Manual observation is fine for 10 agents |
| Anomaly detection | You'll read the logs yourself |

## Configuration

`scenarios/first_winter.toml`:

```toml
[meta]
name = "First Winter"
description = "10 agents, scarce resources, survival pressure"

[world]
width = 10
height = 10
fertile_fraction = 0.3
initial_food_per_fertile = 15
food_regen_rate = 0.1

[agents]
count = 10
starting_food = 10
personality = "random"  # or specify distribution

[simulation]
epochs = 100
snapshot_interval = 10
log_thoughts = true

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
max_tokens = 500
temperature = 0.7
```

## Running the MVP

```bash
# Build
cargo build --release

# Run
./target/release/terrarium --scenario scenarios/first_winter.toml

# Observe
cat output/chronicle.md
less output/events.jsonl
```

## After MVP

If the MVP validates the model, next steps:

1. **Tune**: Adjust parameters for more interesting dynamics
2. **Iterate**: Refine prompts, beliefs, memory
3. **Scale**: 50 agents, then 100
4. **Enrich**: Add terrain variety, seasons, resources
5. **Instrument**: Add metrics, detection, visualization

But first: does the basic model work?

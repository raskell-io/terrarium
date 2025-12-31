# Terrarium

> A societal simulation engine where LLM-powered agents form emergent civilizations.

## Vision

Terrarium is not a game in the traditional sense—it's a **societal petri dish**. Like Conway's Game of Life demonstrated emergent complexity from simple rules, Terrarium explores what emerges when autonomous agents with personalities, memories, and motivations interact within a world governed by consistent rules.

The question driving this project: **Can we observe the organic emergence of social structures, economic systems, and cultural phenomena by giving agents freedom within constraints?**

Inspired by:
- **Simulation games**: Anno, The Settlers, Dwarf Fortress (deep emergent systems)
- **Cellular automata**: Conway's Game of Life (complexity from simple rules)
- **Social sciences**: Sociology, anthropology, behavioral economics
- **Scientific works**: Jared Diamond's "Guns, Germs, and Steel" (environmental determinism, geographical factors shaping societies)
- **Chaos theory**: Sensitive dependence on initial conditions ("butterfly effect")

## Core Philosophy

### The Observer's Paradox
You are God—but a *scientific* God. You set initial conditions, define the rules of physics/economics/society, then **observe without intervention**. The simulation runs, agents make choices, and you read the logs like divine scripture, looking for patterns, testing hypotheses.

### Emergence Over Design
We don't program societies to form markets. We give agents needs (food, shelter, belonging) and capabilities (trade, communication, cooperation), then observe whether markets *emerge*. The invisible hand isn't coded—it's discovered (or not).

### Agents as Genuine Decision-Makers
Each agent is powered by an LLM, giving them:
- **Narrative memory**: They remember their history, relationships, traumas, successes
- **Personality**: Innate traits that bias their decisions (risk tolerance, cooperativeness, ambition)
- **Bounded rationality**: They make "good enough" decisions with incomplete information, like real humans

## Architecture

### The Triad

```
┌─────────────────────────────────────────────────────────┐
│                      OBSERVER                           │
│         (You - God Mode - Analysis & Logs)              │
└─────────────────────────────────────────────────────────┘
                           │
                           │ reads/configures
                           ▼
┌─────────────────────────────────────────────────────────┐
│                   WORLD ENGINE                          │
│                                                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │
│  │   Space     │  │    Time     │  │   Rules     │     │
│  │  (Geography,│  │  (Epochs,   │  │  (Physics,  │     │
│  │   Resources,│  │   Seasons,  │  │   Economics,│     │
│  │   Climate)  │  │   Aging)    │  │   Social)   │     │
│  └─────────────┘  └─────────────┘  └─────────────┘     │
│                                                         │
└─────────────────────────────────────────────────────────┘
                           │
                           │ constrains/informs
                           ▼
┌─────────────────────────────────────────────────────────┐
│                      AGENTS                             │
│                                                         │
│  ┌───────────────────────────────────────────────────┐ │
│  │ Agent                                             │ │
│  │ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌──────────┐ │ │
│  │ │ Body    │ │ Mind    │ │ Memory  │ │ Relations│ │ │
│  │ │(Physical│ │(Persona-│ │(History,│ │(Social   │ │ │
│  │ │ traits, │ │ lity,   │ │ Context,│ │ Graph,   │ │ │
│  │ │ Health, │ │ Values, │ │ Know-   │ │ Trust,   │ │ │
│  │ │ Needs)  │ │ Goals)  │ │ ledge)  │ │ Debts)   │ │ │
│  │ └─────────┘ └─────────┘ └─────────┘ └──────────┘ │ │
│  └───────────────────────────────────────────────────┘ │
│                         × N                             │
└─────────────────────────────────────────────────────────┘
```

### World Engine Components

**Space (Geography)**
- Terrain types with different affordances (fertile plains, defensible hills, resource-rich mountains)
- Resource distribution (á la Diamond: some regions have domesticable plants/animals, others don't)
- Climate zones affecting agriculture, health, movement

**Time (Epochs)**
- Discrete simulation ticks (an "epoch" = a day? a season? configurable)
- Natural cycles: seasons affecting harvests, day/night, aging
- Historical accumulation: events compound over time

**Rules (The Physics of Society)**
- **Physical**: Movement costs, carrying capacity, health/hunger mechanics
- **Economic**: Resource transformation, trade mechanics, property concepts
- **Social**: Communication range, trust mechanics, reputation systems
- **Configurable abstraction levels**: Toggle complexity based on experiment needs

### Agent Components

**Body (Physical Substrate)**
- Health, energy, age
- Physical capabilities (strength, speed, perception)
- Biological needs (hunger, rest, reproduction drive)
- Location in space

**Mind (Personality & Cognition)**
- Big Five personality traits (or similar framework)
- Values and priorities (survival vs. status vs. relationships vs. knowledge)
- Risk tolerance, time preference
- The LLM acts as the "reasoning engine"

**Memory (Narrative History)**
- Episodic memory: Key events in their life
- Semantic memory: Learned facts about the world
- Procedural memory: Skills acquired
- Compressed over time (older memories fade, significant events persist)

**Relations (Social Graph)**
- Known agents and relationship quality
- Trust scores (earned through repeated interactions)
- Obligations, debts, alliances
- Family/kinship structures

### Scale Tiers

| Scale | Population | Focus | Emergent Phenomena |
|-------|------------|-------|-------------------|
| **Tribe** | ~10 | Individual relationships | Leadership, cooperation, conflict |
| **Village** | ~100 | Division of labor | Specialization, early trade, customs |
| **Town** | ~1,000 | Institutions | Markets, governance, religion |
| **City** | ~10,000 | Complex systems | Classes, bureaucracy, culture |
| **Society** | ~100,000 | Civilizational | States, economies, ideologies |

Each tier introduces new dynamics and requires different observation tools.

## The Simulation Loop

```
for each epoch:
    1. WORLD TICK
       - Apply natural processes (resource regeneration, weather, aging)
       - Resolve environmental events (disasters, bounties)

    2. AGENT PERCEPTION
       - Each agent observes their local environment
       - Updates their world model with new information

    3. AGENT DELIBERATION
       - Agent receives: current state, memories, relationships, options
       - LLM generates: reasoning + chosen action
       - Optionally: internal monologue logged for analysis

    4. ACTION RESOLUTION
       - Actions are validated against world rules
       - Conflicts resolved (resource contention, combat, negotiation)
       - State changes applied

    5. CONSEQUENCE PROPAGATION
       - Reputation effects ripple through social networks
       - Economic effects propagate through trade connections
       - Memory updates for all affected agents

    6. LOGGING
       - Full state snapshot (configurable granularity)
       - Event log for significant occurrences
       - Metrics computed for analysis
```

## Hypotheses to Test

The simulation should enable testing questions like:

**Economic**
- Does trade emerge organically when agents have complementary resources/skills?
- Do markets self-regulate, or do they require external enforcement?
- What conditions lead to wealth concentration vs. distribution?

**Political**
- How do leadership structures emerge? (Strength? Charisma? Competence?)
- When does cooperation scale? When does it break down?
- What triggers the formation of in-groups and out-groups?

**Social**
- How do norms and taboos emerge without explicit programming?
- What role does reputation play in enabling cooperation at scale?
- How do belief systems (religions, ideologies) form and spread?

**Geographical (Diamond-style)**
- How does resource distribution affect societal development?
- Does the "right" geography reliably produce more complex societies?
- How do geographical barriers affect cultural divergence?

## Technical Considerations

### LLM Integration
- **Provider agnostic**: Support multiple LLM backends (Claude, GPT, local models)
- **Cost management**: Aggressive caching, tiered model quality based on decision importance
- **Prompt engineering**: Standardized agent prompts with personality/memory injection
- **Batch processing**: Parallel agent deliberation where possible

### Data Model
- **Event sourcing**: All state changes as immutable events
- **Snapshot capability**: Full world state at any epoch for replay/analysis
- **Efficient queries**: Support for historical analysis across many epochs

### Observation Tools
- **Chronicle**: Narrative log of significant events in prose
- **Metrics dashboard**: Population, resources, trade volume, conflict frequency
- **Social graph visualization**: Who knows whom, trust networks
- **Heat maps**: Resource distribution, population density, conflict zones
- **Agent deep-dive**: Full history and psychology of any individual

### Configuration
- **World templates**: Pre-built scenarios (island, continent, resource-scarce, abundant)
- **Rule toggles**: Enable/disable specific mechanics
- **Initial conditions**: Seed populations with specific trait distributions
- **Intervention tools**: For experiments requiring controlled perturbations

## Language & Stack

**Rust** for the core simulation engine:
- Performance critical for large-scale simulations
- Strong type system for complex state management
- Excellent concurrency for parallel agent processing

**Data storage**: SQLite for simplicity, with option to scale to PostgreSQL

**LLM integration**: HTTP clients to various providers, with a unified trait interface

**Observation UI**: Web-based (likely SvelteKit) for visualization and analysis

## Development Phases

### Phase 1: Minimal Viable Terrarium
- Single LLM backend (Claude)
- 10 agents (tribe scale)
- Basic needs: hunger, location, simple resources
- Simple actions: move, gather, trade, communicate
- Text-based chronicle output

### Phase 2: Rich Agents
- Full personality model
- Episodic memory with compression
- Relationship tracking
- Internal monologue logging

### Phase 3: Complex World
- Geography with varied terrain
- Multiple resource types
- Seasonal cycles
- Environmental events

### Phase 4: Scale Up
- Optimization for 100+ agents
- Hierarchical processing (not every agent needs full LLM call every tick)
- Batch LLM calls
- Metrics and analysis tools

### Phase 5: Observation Suite
- Web UI for visualization
- Query interface for historical analysis
- Experiment templating
- Hypothesis testing framework

## Philosophical Underpinnings

### On Determinism vs. Free Will
Each agent has genuine choice (via LLM), but is constrained by:
- Their physical circumstances
- Their personality (consistent biases)
- Their knowledge (bounded rationality)
- The world's rules

This mirrors the human condition: we feel free, but our choices are shaped by factors beyond our control.

### On Validity
This is not a prediction engine. It's a **thought experiment generator**. Results don't prove how human societies work—they show how *one possible* society might work under specific conditions. The value is in generating hypotheses and intuitions, not definitive answers.

### On Ethics
Simulated agents are not conscious (as far as we know), but they are *narrative beings*. Their stories have meaning to us as observers. We should treat the simulation with intellectual honesty—not cherry-picking results, not anthropomorphizing beyond what's warranted, not mistaking the map for the territory.

## Naming Conventions

- **Epoch**: A single simulation tick
- **Chronicle**: The narrative log of events
- **Agent**: An individual entity in the simulation
- **World**: The environment and its rules
- **Terrarium**: The complete simulation instance
- **Observer**: The human running and analyzing the simulation
- **Seed**: Initial conditions for a simulation run

## Open Questions

- How to handle agent death and birth at scale?
- What's the right granularity for an epoch? (Day? Week? Year?)
- How to compress agent memories without losing personality coherence?
- How to detect emergent institutions algorithmically?
- What metrics best capture "societal complexity"?
- How to make LLM costs feasible at 100k agent scale? (Hierarchical abstraction? Representative agents?)

## Getting Started

```bash
# Clone the repository
git clone https://github.com/raskell-io/terrarium.git
cd terrarium

# Build the simulation engine
cargo build --release

# Run a basic tribe simulation
./target/release/terrarium --agents 10 --epochs 100

# View the chronicle
cat output/chronicle.md
```

## Contributing

This is an experimental project exploring the intersection of:
- Agent-based modeling
- Large language models
- Social simulation
- Emergent systems

Contributions welcome in all areas—especially from those with backgrounds in sociology, economics, anthropology, or game design.

---

*"The question is not whether the simulation is realistic. The question is whether it's interesting."*

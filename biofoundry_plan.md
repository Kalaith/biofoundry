# BIOFOUNDRY — Game Plan

*Historical pre-production document — all phases are complete. For the design
of the game as actually implemented (including its genre positioning as a
colony sim / automation hybrid), see [`game_design.md`](game_design.md).*

*Derived from `new_gdd_notes.md` (2026-07-10). Working title: **Biofoundry** (single word, works as crate/package name `biofoundry`, env prefix `BIOFOUNDRY`). Alternatives from the notes: Monsterworks, The Hungry Forge, Goblin Warren — note `monsterhall/` and `monstron/` already exist in this workspace, so avoid "monster*" names.*

---

## 1. Concept

> "An ant colony crossed with a factory builder, where every conveyor belt is a creature with needs."

A top-down 2D factory/colony hybrid where **every piece of automation is a living creature**. Goblins mine and haul, salamanders smelt, slimes eat waste, and a colossal worm is your late-game train line. There is no electricity — **food is the power grid**: every creature has an upkeep draw, farms are generators, cookhouses are power plants, storage is your battery, famine is a brownout, starvation is a blackout.

### Design pillars

1. **Living Automation** — every worker is a creature, not a machine. Workers have position, behavior, and pathing; nothing teleports.
2. **Food Economy** — food replaces electricity. All throughput ultimately drains calories; bigger monsters drain dramatically more.
3. **Biological Progression** — no abstract tech tree. Capture wild species → study them → breed/mutate improved workers.
4. **Ecosystem Pressure** — starvation, morale, overcrowding, disease, and raids create emergent failure. A factory can collapse because the mushroom farm failed.

### What makes it different

| Traditional factory game | Biofoundry |
| --- | --- |
| Machines never complain | Monsters get hungry |
| Power is abstract | Food is tangible, hauled, and stored |
| Belts are deterministic | Creatures have behaviors |
| Upgrades are mechanical | Upgrades are biological |
| Failures are logistical | Failures are ecological |

---

## 2. Core loop

**Gather → Feed → Process → Unlock → Defend → Expand → Repeat** with larger, stranger creatures.

Moment-to-moment, the player:
- designates dig zones and build sites (indirect control — creatures do the work),
- assigns creatures to jobs (miner, carrier, cook, guard, …),
- balances the calorie ledger (production vs. upkeep),
- responds to crises (famine, raids, disease),
- captures and integrates new species that change what automation is possible.

**Control model decision:** indirect control (RimWorld/Dwarf Fortress style job designation), not direct unit orders. This is what makes "living automation" read — you build the habitat and the food chain; the creatures run it.

---

## 3. The food system (the key mechanic — build this first)

Treat food *exactly* like electricity, and surface it like a power UI:

- **Upkeep draw**: every creature consumes `food/min` continuously while assigned to a job (idle creatures draw a reduced rate). Larger tiers draw dramatically more.
- **Generators**: Mushroom Farm +20 food/min (needs water). **Power plants**: Cook Pot converts 10 mushrooms → +30 food/min (cooking multiplies calories — raw food works but is inefficient).
- **Battery**: food stockpiles buffer supply. HUD shows a *calorie balance meter*: production/min, consumption/min, stockpile, and time-to-empty — the exact analogue of a Factorio power graph.
- **Brownout** (hunger): when the stockpile empties, creatures don't die instantly — work speed scales down (e.g. 100% → 50% → 25%), and morale drops. Hungry haulers slow the whole factory: cascading, legible failure.
- **Blackout** (starvation): sustained starvation → creatures stop working, then desert or die (tier-dependent). Guards deserting during a famine invites raids — the ecological death spiral is intentional, but always telegraphed by the meter.
- **Diet chains**: some species don't eat stew. Salamanders need charcoal, beetles need fibrous plants, bats need fruit, drakes need meat. Each new species adds a *supply chain*, not just a stat block — this is the mid-game complexity curve.

Starting balance (all in `assets/data/balance.json`, tunable without recompiling):

| Unit / Building | Produces /min | Consumes /min |
| --- | --- | --- |
| Mushroom Farm | +20 food | water |
| Cook Pot | +30 food | 10 mushrooms |
| Goblin (any job) | — | 1 food (cook/guard: 2) |
| Beetle Hauler | — | 8 fiber-food |
| Drake Furnace | — | 40 (meat + coal) |

The core optimization question at every stage: *is replacing 10 goblins with 1 beetle worth the higher, weirder food cost?* Sometimes yes for throughput; sometimes no during famine.

---

## 4. Creatures

### Creature model

Every creature instance has: species (data-driven), job assignment, hunger/satiation, morale, work speed (derived from hunger+morale), carry capacity, position + path, and (later) traits from breeding. All species definitions, diets, and job aptitudes live in `assets/data/species.json`.

### Tier 0 — The Goblin Warren (early game)

| Unit | Job | Food/min |
| --- | --- | --- |
| Goblin Miner | extract stone/ore | 1 |
| Goblin Carrier | haul items | 1 |
| Goblin Cook | mushrooms → stew | 2 |
| Goblin Guard | defend the warren | 2 |

Early resources: **mushrooms** (food), **water** (farms), **stone** (construction), **scrap ore** (tools).

**The first crisis (scripted by balance, not by script):** the starting goblin count consumes food faster than one mushroom farm produces. The player's first real problem is the hunger grid, ~5 minutes in.

### Tier 1 — Specialized species (mid game)

| Species | Role | Special diet |
| --- | --- | --- |
| Beetle Hauler | hauls 5× a goblin | fibrous plants |
| Salamander Smelter | smelts ore directly (living furnace) | charcoal |
| Slime Janitor | consumes waste/spoilage | water |
| Bat Courier | hauls ignoring terrain | fruit |

Diet chains arrive here: smelting depends on charcoal → wood → lumber goblins/treants → which all eat — industry becomes ecology.

### Tier 2 — Bio-industrial monsters (late game)

| Monster | Factory equivalent | Consumes |
| --- | --- | --- |
| Drake Furnace | electric furnace | meat + coal |
| Ancient Treant | chemical plant / wood grower | water + light shafts |
| Necro-Beast | recycler | corpses |
| Colossal Worm | underground train: creatures enter its mouth, exit at distant outposts | massive food |

Goblin evolution line (breeding): Goblin → Hobgoblin → Goblin Engineer → Goblin Overseer (overseers passively speed nearby workers — the "beacon" analogue).

---

## 5. Progression: Capture → Study → Adapt

No abstract research tree. Unlocks come from *doing biology*:

1. **Capture** — trap or subdue wild species found while expanding (guards + trap buildings).
2. **Study** — a captured specimen in a Study Pen generates knowledge over time (observing behavior).
3. **Adapt** — knowledge + a breeding building unlocks domesticated worker variants, breeding, and mutations.

Example unlock table (data-driven in `assets/data/unlocks.json`):

| Requirement | Unlock |
| --- | --- |
| Capture 3 beetles | Beetle Breeding Pit |
| Smelt 500 ore | Salamander Hatchery |
| Survive a famine | Preservation Techniques (food storage/spoilage upgrades) |
| Defeat a wild drake | Drake Domestication |

Unlock triggers are *event-counters* (things the player already does), so progression is a side effect of playing, and famines/raids double as progression gates.

---

## 6. World & buildings

- **Map**: tile-based underground/surface hybrid. Dig zones carve tunnels (Dwarf-Fortress-lite); surface has forests/water; deeper layers have richer ore and nastier wildlife. Expansion pressure = pillar 4's threat source.
- **Buildings are habitat, not machinery**: burrow tunnels (pathways), feeding troughs (distributed food access — reduce hauling round-trips), breeding dens, study pens, storage heaps, plus producers (mushroom farm, cook pot, charcoal kiln, forge).
- A production block reads like an ecosystem: *treants grow wood → kiln makes charcoal → salamanders smelt ore → goblin smiths forge tools*, and every link eats.
- **Threats** (phased in): hunger (phase 1), wildlife/raids on stockpiles (phase 3), overcrowding & morale (phase 4), disease (phase 5).

---

## 7. Development plan

Phases are ordered so each ends in something playable, testable, and capturable. The prototype gate (end of Phase 1) is the real go/no-go: **if the hunger grid isn't fun with 3 creature types, more species won't save it.**

### Phase 0 — Skeleton (copy `template/`)

- Copy `template/` → `biofoundry/`, follow its README rename steps (package name `biofoundry`, env prefix `BIOFOUNDRY`).
- Module layout per repo standards: `data/` (serde types for species/buildings/balance/unlocks), `simulation/` (stateless tick services), `state/` (GameState machine, save/load), `ui/` (pure view, returns `UiAction`).
- Tile map + camera (toolkit camera), fixed-timestep simulation tick decoupled from render, deterministic state-owned RNG (xorshift like `iron_fauna`).
- Capture scenes wired from day one: `menu`, `warren`.
- Exit: boots, renders a tile map with a movable camera, clippy `-D warnings` clean.

### Phase 1 — The 10-minute prototype (the notes' explicit slice)

Scope, verbatim from the notes — keep it tiny:
- **3 creatures**: Goblin Miner, Goblin Carrier, Goblin Cook (toolkit pathfinding for movement).
- **3 resources**: Ore, Mushrooms, Food.
- **1 building**: Mushroom Farm (plus an implicit Cook Pot as the cook's workstation).
- **1 threat**: Hunger — full brownout/blackout pipeline from §3, with the calorie balance HUD.
- **1 upgrade**: Beetle Hauler (proves the "expensive specialist vs. cheap generalist" decision).
- **Win condition**: maintain a 100-food surplus while producing 50 processed ore. Show a victory screen; keep playing after.
- Tests: calorie-ledger math, hunger state transitions, job assignment, a full sim-to-win integration test on a fixed seed.
- Exit gate: **playtest — is the hunger grid tense and legible?** Tune `balance.json` until the first famine hits ~5 min in and is survivable by reassigning workers. Only proceed if this loop is fun.

### Phase 2 — Real factory: building placement, hauling economy, diet chains

- Player-placed buildings (build menu → ghost → carriers deliver materials → built). Dig designations carve tunnels.
- Item stockpiles + hauling jobs as a real logistics layer (items physically move; hauler throughput matters).
- Tier 1 species with distinct diets: Salamander Smelter (charcoal chain: wood → kiln → charcoal), Slime Janitor (waste/spoilage introduced here), Bat Courier.
- Food variety: raw vs. cooked multiplier, spoilage, feeding troughs.
- Save/load of the full sim (toolkit persistence), roundtrip test.
- Capture scenes: `factory`, `famine` (staged low-food state).

### Phase 3 — Capture / Study / Adapt + defense

- Wild creatures spawn at map edges/depths; traps and guard jobs; capture mechanic.
- Study Pens + unlock counters (`unlocks.json`) driving all progression from here on.
- Breeding dens: goblin evolution line (Hobgoblin/Engineer/Overseer) as the first breeding payoff.
- Raids: periodic wildlife attacks targeting food stockpiles (they're hungry too) — guards, walls, and the desertion spiral from §3 become live.
- Capture scenes: `raid`, `breeding`.

### Phase 4 — Bio-industrial late game

- Tier 2 monsters: Drake Furnace, Ancient Treant, Necro-Beast, and the **Colossal Worm** (point-to-point living transit between outposts — the flagship late-game moment).
- Multi-outpost play: expand to remote ore fields, worm-connected.
- Morale + overcrowding systems; overseers/handlers as the management layer.
- A soft campaign goal (e.g. "awaken/feed the Colossal Worm" as a victory monument) + endless mode.

### Phase 5 — Polish & publish

- Audio (toolkit `SoundManager`, synthesized WAVs as in `iron_fauna`), menu/title art, `catalog_thumbnail.png` (16:9 title capture).
- Balance pass across all tiers (everything already in JSON), difficulty settings.
- Full verification: `cargo test`, clippy `-D warnings`, WASM build via `publish.ps1 -DryRun`, all capture scenes green.

---

## 8. Technical notes (repo-standard)

- **Simulation determinism**: fixed-timestep tick; all randomness through state-owned RNG; sim logic in stateless `simulation/` services (take state, return results) so integration tests can run headless for thousands of ticks.
- **Data-driven everything**: `assets/data/species.json`, `buildings.json`, `resources.json` (incl. diet tags), `unlocks.json`, `balance.json`. Native reads from disk, `include_str!` fallback for WASM (template pattern). Tests validate all cross-references (species diets name real resources, unlock rewards name real buildings).
- **UI is a view**: panels read state and emit `UiAction` (`Designate(rect, Dig)`, `Assign(creature, Job)`, `PlaceBuilding(kind, pos)`, …); an actions dispatcher applies them.
- **Performance guardrail**: hundreds of pathing creatures is the risk. Cache paths, re-path only on map change or arrival, stagger creature AI updates across frames. Toolkit pathfinding first; only specialize if profiling demands it.
- **800-line limit**: creature AI, hauling, and the food ledger each get their own module from the start — these are the files that balloon.

## 9. Open design questions (resolve during Phase 1–2 playtests)

1. **Death vs. desertion**: does starvation kill (harsh, Dwarf-Fortress) or make workers leave (softer, recoverable)? Plan assumes tier-dependent: goblins desert, exotic monsters go feral and become a threat. Playtest it.
2. **Direct feeding vs. self-feeding**: do creatures walk to troughs (more logistics depth, more pathing cost) or is food deducted from the nearest stockpile (cheaper, more abstract)? Prototype self-feeding from troughs *by zone* as a middle ground.
3. **Combat depth**: raids could be a stat check (defense points vs. raid points) or real-time skirmishes. Start with the simple version; the game is about the factory.
4. **Map size/persistence of multiple outposts** — defer until Phase 4; the worm mechanic decides it.

## 10. Success criteria

- Phase 1 gate: a first-time player hits the famine, understands *why* via the calorie meter, fixes it, and wins the 50-ore goal in ~10–15 minutes.
- Ship gate: all phases' capture scenes render correctly, full test suite green, clippy clean, WASM ≤ ~4 MB, playable start-to-worm in one sitting.

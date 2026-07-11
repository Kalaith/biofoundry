# Biofoundry — Game Design

*This document describes the game **as implemented** (all plan phases 0–5
complete, July 2026). For the original phase plan and pre-production notes see
[`biofoundry_plan.md`](biofoundry_plan.md) and
[`new_gdd_notes.md`](new_gdd_notes.md). All numbers quoted here come from
`assets/data/*.json` — that's the source of truth, and it's tunable without
recompiling.*

> "An ant colony crossed with a factory builder, where every conveyor belt is
> a creature with needs."

---

## 1. Genre & positioning

Biofoundry started from a factory-builder pitch, but what emerged is a
**colony simulation with an automation backbone** — closer to RimWorld or
Dwarf Fortress than to Factorio, and deliberately so.

What it takes from each tradition:

| From factory builders (Factorio) | From colony sims (RimWorld / DF) |
| --- | --- |
| A production/consumption ledger you optimize (the food grid HUD is a power graph) | Indirect control: you designate, assign, and place — creatures decide how |
| Production chains (ore → metal via wood → charcoal → smelter) | Workers with needs, hunger states, and desertion |
| Buildings as throughput nodes with input/output rates | Emergent crises: famine spirals, raids on the larder |
| Explicit "factory complete" style goals | A living map: wild creatures wander in, capture/study progression |

There are **no belts, no inserters, no direct unit orders, and no blueprints**.
Logistics is creatures walking: a "conveyor" is a goblin carrier (or a beetle)
physically hauling one load at a time along BFS paths. The optimization game is
real — where you place farms relative to the cook pot changes throughput — but
you optimize by shaping a habitat and a workforce, not by routing machinery.

The honest one-liner: **a simulation game about running an economy of hungry
creatures, played with factory-game instincts.** RimWorld colonies become
automation engines by the late game; Biofoundry starts from that end state and
makes the automation itself alive.

## 2. Design pillars (as shipped)

1. **Living automation.** Every unit of throughput is a creature with a
   position, a path, a stomach, and a job. Nothing teleports; nothing is
   abstract. If the cook is hungry, stew production physically slows.
2. **Food is the power grid.** There is no electricity. Every creature has an
   upkeep draw in food/min; farms are generators, cook pots are power plants,
   the stockpile is the battery, famine is a brownout, starvation is a
   blackout. The HUD surfaces this exactly like a power UI.
3. **Progression is biological.** No research tree. Unlocks fire from event
   counters on things the player already does: capture creatures, survive
   crises, forge metal.
4. **Ecosystem pressure.** The world pushes back — wild beetles wander in,
   gnarl raiders eat your larder, and your own hungry guards can desert at the
   worst moment. Failure is ecological, and always telegraphed by the meter.

## 3. Player verbs (the control model)

The player never orders a specific creature to a specific tile. The full verb
set:

- **Assign jobs** — a Jobs panel with −/+ counters per job (Miner, Carrier,
  Cook, Guard); goblins move between jobs and the idle pool. This is the
  primary crisis-response lever: shifting miners into carriers before the
  famine is the game's first learned skill.
- **Designate digs** — Dig mode toggles designations on rock; miners carve the
  tunnels (and the ore veins inside them) on their own time.
- **Place buildings** — Build mode drops a ghost; carriers haul banked ore to
  the site; it becomes real when supplied. Placement is the layout game:
  farms near the cook pot mean shorter hauls and faster stew.
- **Attract specialists** — spend banked ore to attract a Beetle Hauler
  (25 ore) or a Salamander Smelter (20 ore).
- **Inspect** — click creatures/buildings for status.
- **Save/load** — F5/F9, full-simulation snapshot.

Everything else — pathing, hauling priorities, eating, fighting, fleeing — is
the simulation's job.

## 4. The food grid

The central mechanic. Every creature drains satiation continuously and refills
from the stockpile; the stockpile is fed by the farm → haul → cook chain.

- **Upkeep**: a working goblin draws 2 food/min. Cooks and guards work harder:
  ×2 (4/min). Idle creatures draw ×0.5. A beetle draws 8/min — the classic
  "is one expensive specialist worth ten cheap generalists?" decision.
- **Generation**: a Mushroom Farm grows 14 mushrooms/min (local cap 24;
  carriers must haul them or the farm idles). A Cook Pot converts 2 mushrooms
  → 3 food per 6-second batch — cooking multiplies calories, so the kitchen is
  the power plant, not the farm.
- **Battery & meter**: the HUD's calorie ledger shows production/min,
  upkeep/min, stockpile, and **time-to-empty** — the Factorio power graph,
  reskinned.
- **Brownout**: when the stockpile empties, creatures don't die — work speed
  degrades as they starve. Hungry carriers slow the kitchen, which deepens the
  famine: a legible cascade.
- **Blackout**: a creature starving for 90 continuous seconds deserts the
  warren. Guards deserting during a famine is how raids become lethal — the
  death spiral is intentional and visible on the meter.
- **Load-shedding**: carriers triage. Below a 40-food reserve they serve the
  kitchen first; above it, industry (wood, charcoal, construction). The player
  feels this as the factory "dimming" during food stress.
- **Recovery**: refeeding fully restores work speed; surviving a famine
  (recovering to 20+ food) increments the `famines_survived` counter and
  unlocks Preservation Techniques.

On the default seed the first famine hits **~5.5 sim-minutes in** and is
survivable by reassigning workers — this is the tuned first crisis, taught by
the tutorial.

## 5. Creatures

All species live in `assets/data/species.json`.

| Species | Diet | Upkeep | Carry | Role |
| --- | --- | --- | --- | --- |
| Goblin | food | 2/min (×2 cook/guard, ×0.5 idle) | 1 | The generalist. Reassignable between Miner / Carrier / Cook / Guard / Idle. |
| Beetle Hauler | food | 8/min | 5 | Dedicated hauler, 5× a goblin's load. Attracted for 25 ore; not reassignable. |
| Salamander Smelter | **charcoal** | — | 0 | A living furnace: its meal is its fuel. Attracted for 20 ore; works the Smelter Den. |
| Wild Beetle | — | — | — | Wanders in from the map edge (every ~100 s, max 2 loose). Capturable in snare traps. |
| Gnarl Raider | your stockpile | — | — | Raid antagonist: beelines for the larder and eats 30 food/min until driven off or sated (flees after 12). |

Jobs (goblins only): **Miner** (digs designations, mines ore veins — 14 ore
per vein, 8 s per mining op), **Carrier** (hauls mushrooms, wood, charcoal,
ore, and building materials), **Cook** (runs the pot), **Guard** (8 DPS,
patrols the stockpile, intercepts raiders; fed creatures regenerate HP —
starving guards lose fights), **Idle** (reserve pool, half upkeep).

## 6. Buildings & production chains

Buildings cost banked ore, are placed as ghosts, and are built when carriers
deliver materials. From `assets/data/buildings.json`:

| Building | Ore | Purpose |
| --- | --- | --- |
| Mushroom Farm | 10 | Grows mushrooms (food chain input). |
| Cook Pot | 8 | Mushrooms → stew (the calorie multiplier). |
| Charcoal Kiln | 12 | Sporewood → charcoal, 3/min (wood cap 8). |
| Smelter Den | 15 | Salamander workstation: 1 ore + 1 charcoal → 1 metal per 10 s batch. |
| Snare Trap | 6 | Single-use wild-beetle capture. |
| Study Pen | 12 | Captured specimens generate knowledge (1/min each) and drive unlock counters. |
| Breeding Pit | 20 | *Unlocked by studying beetles.* Hatches free beetle haulers every 150 s (cap 3). |
| Worm Shrine | 20 | *Unlocked by forging 20 metal.* The campaign monument (see §8). |
| Stockpile | — | The battery/larder (starts on the map; raid target). |

The three chains, each of which **eats**:

1. **Food (the grid):** farm grows mushrooms → carriers haul → cook pot makes
   stew → stockpile → every stomach in the warren.
2. **Ore (construction & goals):** miners carve designations and veins →
   carriers deliver ore → banked ore pays for buildings and attracting
   specialists.
3. **Metal (the diet chain):** sporewood groves (regrow ~45 s) → carriers haul
   wood → kiln smoulders charcoal → salamander eats the charcoal *as its
   smelting fuel* → metal. Industry depends on forestry depends on hauling
   depends on food — the mid-game complexity curve in one chain. Smelters draw
   banked ore only above a 12-ore reserve (with an emergency trickle) so
   endless metal can't starve construction.

## 7. Progression: capture → study → adapt

No tech tree. `assets/data/unlocks.json` defines event-counter unlocks —
progression is a side effect of playing, and crises double as gates:

| Counter | Threshold | Unlock |
| --- | --- | --- |
| Beetles captured | 2 | **Beetle Breeding Pit** (free hauler production) |
| Raids survived | 1 | **Hardened Guards** (guard DPS ×1.5) |
| Famines survived | 1 | **Preservation Techniques** (farm storage ×1.5) |
| Metal forged | 20 | **Worm Shrine** (the endgame) |

The capture loop: wild beetles wander the map → place snare traps in their
path → captured specimens go to Study Pens → knowledge accumulates → counters
tick → unlocks fire with a toast.

## 8. Threats & the campaign arc

**Threats.** Famine (from ~5.5 min, recurring whenever the ledger goes
negative) and **gnarl raids**: first at 9 minutes, then every ~6.3 minutes,
up to 3 raiders who eat the stockpile directly. Guards fight them off; the
raid *is* a food-grid event (raiders are hungry too), and the famine ↔
desertion ↔ raid loss spiral is the game's failure state.

**The arc**, on the fixed default seed (full-campaign probe test):

| Beat | ~Sim time | What it asks of the player |
| --- | --- | --- |
| First famine | 5.5 min | Read the meter, reassign jobs. |
| **Warren Secured** (win 1) | ~15 min | Hold a 100-food surplus + deliver 50 ore. |
| **Factory Complete** (win 2) | ~26 min | Build the charcoal chain, forge 20 metal. |
| **Worm Awakened** (campaign) | ~49 min | Build the Worm Shrine; feed the Colossal Worm 60 food of offerings. |

The Worm Shrine is the endgame stress test: the worm demands offerings at
**12 food/min** — a permanent heavy draw on the grid — but pauses politely
below a 25-food reserve, so it pressures the economy without being able to
blackout the warren by itself. Sating it awakens the worm, which coils around
its shrine as a monument; **endless mode** continues after, with ongoing raid
pressure. The campaign is one sitting (~49 minutes), losing at most a worker
or two if played reactively.

## 9. Onboarding

A six-step tutorial (`assets/data/tutorial.json`) shows as a HUD card in new
warrens. Every step advances on a **real player action**, not a timer or a
"next" button: move the camera → watch the food grid → reassign a job →
survive the first famine → place a building → win. Skippable; progress
persists in saves.

## 10. World & presentation

- **Map**: 48×32 tiles, seeded deterministic generation (default seed
  `20260710`) — rock, dug tunnels, ore veins, mushroom patches (regrow 90 s),
  sporewood groves, water. Dig designations expand the warren
  Dwarf-Fortress-lite.
- **Camera**: toolkit pan/zoom (WASD / right-drag / scroll).
- **Audio**: 7 synthesized SFX WAVs via toolkit `SoundManager` (build, eat,
  raid, victory, …), degrading to silence headless.
- **Title screen**: worm silhouette, mushroom clusters, drifting spores;
  captured as `catalog_thumbnail.png`.

## 11. Technical shape (summary)

Repo-standard architecture; see `README.md` and `docs/`:

- Fixed-timestep simulation decoupled from render; state-owned seeded RNG;
  stateless `simulation/` services (`food`, `jobs`, `nav`, `wildlife`), so
  integration tests run the sim headless for thousands of ticks — including
  a full-campaign probe on the fixed seed that guards the ~49-minute arc.
- **Everything balance-related is JSON** under `assets/data/` (`balance.json`,
  `species.json`, `buildings.json`, `unlocks.json`, `tutorial.json`,
  `game_config.json`) — edit the JSON, not Rust constants.
- UI is a pure view layer emitting `UiAction` intents; a dispatcher applies
  them. Headless capture scenes (`menu`, `warren`, `factory`, `famine`,
  `raid`, `breeding`, …) verify the UI without interactive input.
- Full save/load of the live sim (F5/F9, toolkit persistence).

## 12. Backlog / future directions

Deferred from the original plan, in rough order of design interest:

- **Slime Janitor + spoilage/troughs** — waste as a resource sink; troughs as
  distributed food access (deepens the logistics-vs-abstraction question).
- **Bat Courier** — terrain-ignoring hauling as a topology counter-play.
- **Morale & overcrowding** — the second colony-sim pressure axis beyond
  hunger.
- **Multi-outpost worm transit** — the original "living train line" fantasy:
  creatures enter the worm's mouth, exit at remote outposts. This is the
  feature that would pull the game meaningfully back toward the automation
  genre.
- **Goblin evolution line** (Hobgoblin → Engineer → Overseer) — breeding
  depth beyond beetles; overseers as living "beacons".
- **Worm Shrine pause-feeding toggle** — small QoL on the endgame draw.

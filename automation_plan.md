# BIOFOUNDRY — Automation Plan (Phases 6–11)

*Forward plan, picking up where [`biofoundry_plan.md`](biofoundry_plan.md)
(phases 0–5, all shipped) left off. The game as implemented is documented in
[`game_design.md`](game_design.md); this plan changes it, so that document
gets updated at the end of each phase.*

---

## 1. Why this plan exists

The vertical slice lands the **colony-sim half** of the pitch: the food grid,
famine, jobs, raids. What it under-delivers is the **automation half**. Today,
mining is goblins roaming dig designations, and the first real production
chain (wood → charcoal → smelter → metal) doesn't open until win 2 territory,
~15+ minutes in. That's RimWorld pacing: automation is the late-game reward.

The correction: **automation is the early game.** Within the first five
minutes the player should be able to read this sentence on their own map:

> A goblin works the Mine. A carrier walks its ore to the Blacksmith. The
> Blacksmith hammers ingots. Ingots become a better pickaxe, and the Mine
> speeds up.

That's a factory loop — extraction → logistics → processing → **feedback into
extraction** — built entirely out of creatures. Every phase below serves it.

### What changes

- Ore extraction moves from roaming designation-miners to a **staffed
  workstation** (the Mine) with a visible rate and buffer.
- A **Blacksmith** turns ore into **ingots** from the early minutes — the
  charcoal/salamander chain becomes the *bulk upgrade* to smithing, not the
  introduction of processing.
- **Equipment** crafted from ingots feeds back into throughput (pickaxes,
  hauling frames, guard blades) — the biological answer to Factorio modules.

### What is sacred (unchanged pillars)

- **Indirect control.** The player places buildings and sets job counts;
  creatures claim workstations themselves. Never "order goblin #7 to tile X".
- **Food is the power grid.** Every new node eats. A bigger factory is a
  bigger draw; the calorie ledger stays the master constraint.
- **Nothing teleports.** Ingots ride on legs. Logistics stays physical.
- **Data-driven.** Every rate, cost, and recipe in `assets/data/*.json`;
  the fixed-seed campaign probe test keeps guarding the arc.

---

## 2. The target early game (minute-by-minute sketch)

| ~Min | Beat |
| --- | --- |
| 0–2 | Tutorial: look around, place a Farm, meet the food grid. A prebuilt Mine is already ticking beside the warren. |
| 2–4 | Place the Blacksmith. Carrier hauls mine ore to it; first ingots. |
| 4–6 | First famine (unchanged) — the food grid interrupts the factory. |
| 6–9 | Order a pickaxe. Mine speeds up visibly. Place a second Mine on a dug-out vein. |
| 9+ | Raids, capture, charcoal chain, salamander bulk smelting — as today, but now they upgrade a factory the player already runs. |

The design bet mirrors the Phase 1 gate: **if the mine → blacksmith → pickaxe
loop isn't satisfying by minute 8, later content won't save it.** Phase 8 ends
with that playtest.

---

## 3. Phases

### Phase 6 — Workstations: the Mine

*Extraction becomes a building you place and staff. The Farm already works
this way (grows into a local buffer, idles when full, carriers drain it) —
this phase promotes that into the universal workstation pattern.*

- **Mine building** (`buildings.json`): placeable only on/adjacent to an ore
  vein. Extracts continuously while staffed — rate, local buffer cap, and
  remaining vein reserve all in `balance.json`. Reserves are generous (a mine
  is infrastructure, not a chore) but finite, so expansion pressure survives.
- **Stationing**: a `workstation` field on building defs (job + slots). Idle
  or job-matching creatures claim the nearest open slot on their own; the
  Jobs panel keeps working exactly as today (Miner count = how many goblins
  want a mine slot). Unstaffed workstation = stopped node, shown at a glance.
- **Migration**: roaming vein-mining is deleted, not kept as a fallback (two
  extraction systems = incoherent). Dig designations remain the expansion
  verb — digging uncovers veins; mines exploit them. The starting map gets a
  **prebuilt Mine** next to the warren (alongside the existing farm/pot/
  stockpile) so the loop is alive at second zero.
- **Inspection panel** (click a building): worker, rate/min, buffer, reserve.
  First pass — Phase 9 builds the full legibility layer on top.
- Tests: mine staffing/claiming, extraction-into-buffer math, reserve
  depletion, probe test still reaches win 1 on the fixed seed.
- Capture scene: `mine` (staffed mine mid-extraction, inspection open).
- **Exit gate**: place a mine, watch a goblin claim it and ore flow to the
  stockpile with zero further input. Ore/min visible on inspect.

### Phase 7 — The Blacksmith and ingots

*The first processing node, in the first minutes — and one resource identity:
"metal" is renamed **ingots** everywhere.*

- **Blacksmith building** (~8 ore, no unlock): a goblin **Smith** job (new
  Jobs panel row, cook-tier ×2 upkeep). Recipe: N ore → 1 ingot per batch,
  slow — deliberately worse per-ore than the salamander smelter so the
  charcoal chain stays the mid-game throughput upgrade (burner furnace →
  electric furnace, in living form). The smelter's output is renamed to
  ingots; win 2 ("forge 20 metal") becomes "forge 20 ingots", threshold
  retuned since ingots now start flowing at ~minute 4.
- Carriers learn the route: mine buffer → blacksmith input → ingot output →
  stockpile. Reuses the existing haul-priority system (kitchen still wins
  below the food reserve — a starving warren *should* dim its industry).
- Win-1 goal check: "deliver 50 ore" likely becomes "bank 50 ore **or**
  forge N ingots" — decide in playtest; the probe test pins the result.
- Tests: smith batch math, chain integration (mine → blacksmith → ingot with
  only carriers in between), probe test updated for the retuned win 2.
- Capture scene: `blacksmith` (chain mid-flow).
- **Exit gate**: on a fresh warren, first ingot banked by ~minute 4 with only
  two player actions (place blacksmith, +1 Smith).

### Phase 8 — Equipment: the feedback loop

*Ingots become gear; gear makes the factory faster. This is the phase that
makes it an automation game.*

- **`equipment.json`**: data-driven items with a job affinity and a modifier.
  Launch set:

  | Item | Cost | Effect |
  | --- | --- | --- |
  | Iron Pickaxe | 2 ingots | Mine extraction ×1.5 (equipped miner) |
  | Hauling Frame | 2 ingots | Carrier capacity +1 |
  | Smith's Hammer | 3 ingots | Blacksmith batch time ×0.75 |
  | Guard Blade | 3 ingots | Guard DPS ×1.5 (stacks with Hardened Guards) |

- **Production orders**: the Blacksmith inspection panel gets a small queue
  ("craft: Pickaxe ×2") — the player's first explicit production verb. Smiths
  work the queue after (or instead of — playtest) passive ingot batches.
- **Auto-equip, indirect**: finished gear goes to the stockpile; creatures
  pick up gear matching their job on their next visit. No per-creature
  micromanagement, ever. Reassigned goblins drop job-mismatched gear back.
  No durability at launch (upkeep chore) — revisit only if gear feels too
  fire-and-forget.
- Creatures render their gear (pickaxe glint, frame silhouette) so an
  upgraded workforce is *visible*.
- Tests: modifier math, equip/drop-on-reassign rules, order queue,
  save/load roundtrip with equipped creatures.
- Capture scene: `equipment` (equipped miner at a mine, order queue open).
- **Exit gate — the plan's go/no-go playtest**: by minute 8 a player has
  crafted a pickaxe and *felt* the mine speed up (ore/min on the inspection
  panel before/after). If this loop isn't satisfying, stop and retune before
  building anything below.

### Phase 9 — Factory legibility

*An automation game is only as good as its readability. Make every stalled
node diagnosable in one glance — the equivalent of Factorio's no-power icon.*

- **Status icons over buildings**: no worker / input starved / output full /
  no vein reserve / (farm) awaiting haul. Color-coded, drawn in-world.
- **Chain throughput panel**: per-resource flow rates (ore mined/min, ingots
  forged/min, food cooked/min) from the same EMA machinery as the food
  ledger — the food grid generalized into a factory dashboard.
- **Haul demand readout**: how many pending haul jobs per chain, so "add a
  carrier" becomes a read decision instead of a guess.
- Stockpile QoL as playtests demand (e.g. a per-resource cap or a "don't
  haul" toggle) — smallest thing that removes confusion, no zone-designation
  system unless the need is proven.
- Tests: status derivation per building state; screenshot scenes assert the
  icons render (`warren` and `factory` scenes updated).
- **Exit gate**: stall any chain link (unstaff the mine, plug the output,
  starve the kiln) and a playtester can name the problem within seconds
  without clicking anything.

### Phase 10 — Living machines: the goblin evolution line

*Backlog item promoted: breeding depth beyond beetles, framed as automation.
Gear upgrades the tool; breeding upgrades the creature holding it.*

- **Hobgoblin** (bred at the Breeding Pit from studied goblins? — unlock
  counter TBD, e.g. "forge 30 ingots" + knowledge): a heavyweight worker,
  ×2 work speed, ×2.5 upkeep — the specialist-vs-generalist ledger question
  again, now for labor itself.
- **Goblin Overseer**: doesn't work; radiates a work-speed aura around its
  post (the living beacon). One per district; expensive upkeep.
- Equipment × species multipliers stack (hobgoblin + pickaxe = the late-game
  mine), so the two upgrade axes multiply rather than compete.
- Unlocks stay event-counter-driven (`unlocks.json`); no tech tree creep.
- Tests: aura math, breed unlock counters, ledger balance sims.
- Capture scene: `overseer`.
- **Exit gate**: a mature warren visibly runs on fewer, better creatures than
  a mid-game one at the same throughput — count the legs on screen.

### Phase 11 — Arc rebalance, tutorial rewrite, publish

- **Campaign retune** on the fixed seed: famine ~5 min, win 1 ~12–15 min,
  win 2 (ingots) ~22–26 min, worm ~45–50 min — same silhouette, but every
  beat now passes through the factory loop. The probe test is the contract;
  retune `balance.json` until it holds.
- **Tutorial rewrite** (`tutorial.json`): the current 5 steps become ~7,
  action-gated as always — look around → place a Farm → meet the Mine (it's
  already working) → place the Blacksmith → reassign for the famine → craft
  a pickaxe → win. The equipment step is the new teaching centerpiece.
- Worm offerings audit: the endgame draw should stress the *whole* factory
  (food and ingot chains), not just the kitchen — consider a mixed offering.
- `game_design.md` fully updated; `standing.md` note; fresh capture scenes;
  new `catalog_thumbnail.png` if the title composition changed.
- Full verification: `cargo test`, clippy `-D warnings`, WASM build,
  `publish.ps1 -DryRun`, all capture scenes green.
- **Exit gate**: a first-time player runs mine → blacksmith → pickaxe without
  reading anything but the tutorial card, and the fixed-seed campaign probe
  passes at the retuned timings.

---

## 4. Data & schema changes (accumulated)

| File | Change |
| --- | --- |
| `buildings.json` | `mine`, `blacksmith`; new `workstation: { job, slots }` field; prebuilt-at-start flag for the starting Mine |
| `equipment.json` | **new** — items: job affinity, ingot cost, modifier |
| `species.json` | `hobgoblin`, `overseer` (Phase 10) |
| `balance.json` | mine rate/buffer/reserve, smith batch, order queue size, aura radius/multiplier, retuned win thresholds |
| `unlocks.json` | ingot-counter unlocks (hobgoblin, overseer) |
| `tutorial.json` | rewritten in Phase 11 |
| save format | equipment on creatures, workstation claims, order queues — version-migrated via the existing toolkit path |

Renames: resource `metal` → `ingot` (Phase 7), with save migration.

## 5. Open design questions (resolve in playtests, not up front)

1. **Smith default behavior** — passively batch ingots when the order queue
   is empty, or only work orders? (Passive is more "automation"; orders-only
   is more legible. Start passive.)
2. **Mine reserves** — finite-but-large vs infinite-with-falloff. Start
   finite; if replacing mines reads as busywork, switch.
3. **Win 1 shape** — does "bank 50 ore" survive, or does the first goal
   become ingot-denominated? Playtest at Phase 7.
4. **Gear scarcity** — is equipment per-creature persistent through
   desertion/death? (A deserting miner walking off with the pickaxe is
   thematically perfect and mechanically cruel. Probably yes.)
5. **Blacksmith fuel** — launch with labor-only smithing; if the early game
   feels *too* frictionless, a small sporewood cost is the lever.

## 6. Success criteria

- **Minute 8**: mine → blacksmith → pickaxe loop closed on a fresh warren
  (Phase 8 gate — the go/no-go).
- **One glance**: any stalled chain link is diagnosable from its status icon
  (Phase 9 gate).
- **Ship**: retuned campaign probe green on the fixed seed, all scenes
  captured, clippy clean, WASM within budget — and the honest one-liner
  upgrades from "a simulation game played with factory instincts" to **"a
  factory game whose machines are alive."**

# Biofoundry

> "An ant colony crossed with a factory builder, where every conveyor belt
> is a creature with needs."

A top-down 2D factory/colony hybrid where **every piece of automation is a
living creature** and **food is the power grid**: farms are generators,
cookhouses are power plants, stockpiles are batteries, famine is a brownout,
and starvation is a blackout.

Design and phase plan: see [`biofoundry_plan.md`](biofoundry_plan.md).

## Status

- **Phase 0 (done)**: package skeleton, tile-map warren generation
  (deterministic seeded RNG), toolkit camera pan/zoom, fixed-timestep
  simulation tick, menu/warren state machine, capture scenes.
- **Phase 1 (done)**: the 10-minute prototype. Goblin miners/carriers/cooks
  with BFS pathfinding, the food grid (farm → haul → cook → stockpile),
  hunger brownout/blackout with desertion, calorie balance HUD with
  time-to-empty, job reassignment, beetle hauler upgrade, and the win
  condition (100-food surplus + 50 ore). On the default seed the first
  famine hits ~5.5 sim-minutes in and a reactive player wins ~13.6 minutes.
- **Phase 2a (done)**: player-placed buildings (ghost → carriers haul ore →
  built), dig designations carved by miners, multi-building logistics
  (several farms/pots, per-building stock), banked-ore economy, full-sim
  save/load (F5/F9, toolkit persistence), capture scenes `factory` and
  `famine`.
- **Phase 2b (done)**: the first diet chain. Sporewood groves → carriers
  haul wood → Charcoal Kiln smoulders it into charcoal → the Salamander
  Smelter (attracted for ore, eats charcoal — its meal is its fuel) forges
  ore into metal. Carriers load-shed: kitchen first when food dips below
  reserve, industry otherwise. Extended goal: forge 20 metal ("Factory
  Complete"). Full-run probe on the fixed seed: famine ~5.5 min → first
  victory ~14 min → **factory complete ~34 sim-minutes** — a full sitting.
- **Phase 3 (done)**: the living world bites back. Wild beetles wander in
  (snare traps capture them — single-use), study pens generate knowledge,
  and `unlocks.json` drives progression through event counters: capture 2
  beetles → Breeding Pit (hatches free haulers), survive a raid →
  Hardened Guards, survive a famine → Preservation (bigger farms). Gnarl
  raiders periodically attack the larder and eat the stockpile; the Guard
  job fights them off (fed creatures regenerate; starving guards lose —
  the desertion spiral is real). Full-run probe with defense in the mix:
  factory complete ~25 sim-minutes plus endless raid pressure after.
- **Phase 4 (done)**: the Colossal Worm campaign monument. Forging 20
  metal unlocks the Worm Shrine; once built, the worm demands food
  offerings (a 12/min power draw that pauses below a reserve so it can't
  blackout the warren alone). Sating it awakens the worm — campaign
  complete, endless mode continues with the beast coiling around its
  shrine. Smelters now draw banked ore only above a reserve (with an
  emergency trickle) so endless metal can't starve construction. Full
  campaign probe: famine 5.5 min → victory ~15 → factory ~26 → **worm
  awakened ~49 sim-minutes**, at most one worker lost.
- **Phase 5 (done)**: synthesized SFX bank (7 WAVs via toolkit
  `SoundManager`, loaded from the published asset pack, degrading to
  silence headless), title-screen backdrop (worm silhouette, mushroom
  clusters, spores), `catalog_thumbnail.png` (16:9 title capture), and
  full publish verification: 34 tests, clippy `-D warnings`, WASM 1.29 MB,
  `publish.ps1 -DryRun` green end-to-end, all seven capture scenes
  verified.

- **Post-plan**: a six-step tutorial (`assets/data/tutorial.json`) shown
  as a HUD card in new warrens — each step auto-advances when the player
  actually does the thing (look around, reassign a job, weather the
  famine, place a building, win). Skippable; progress persists in saves.

**All plan phases (0–5) are complete.** Deferred backlog: slime janitor,
bat courier, spoilage/troughs (plan §Phase 2), morale/overcrowding and
multi-outpost worm transit (§Phase 4), and a pause-feeding toggle for the
Worm Shrine.

## Run

```powershell
cargo run
```

## Test / lint

```powershell
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## Screenshots (headless capture)

```powershell
./scripts/capture_ui.ps1   # menu, warren, factory, famine, raid, breeding
```

Writes PNGs to `docs/verification/`. Uses the `BIOFOUNDRY_CAPTURE_*` env-var
hook from `macroquad_toolkit::capture`.

## Publish

```powershell
./publish.ps1          # Windows + WebGL build and deploy
./publish.ps1 -DryRun
```

## Module layout

- `data/` — serde types for config/species/buildings/balance, embedded JSON
  from `assets/data/` (edit the JSON, not Rust constants).
- `simulation/` — stateless fixed-timestep tick services.
- `state/` — `GameState` machine, live `GameSession`, world map.
- `ui/` — pure view layer; reads state, returns `UiAction` intents.

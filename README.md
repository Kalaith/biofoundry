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
- **Phase 2b (next)**: Tier 1 species with distinct diets (salamander
  smelter + charcoal chain, slime janitor, bat courier), food variety and
  spoilage, feeding troughs.

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
./scripts/capture_ui.ps1 -Scenes menu,warren,factory,famine
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

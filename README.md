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
- **Phase 1 (next)**: the 10-minute prototype — goblins, jobs, the hunger
  grid, calorie balance HUD, beetle hauler upgrade, win condition.

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
./scripts/capture_ui.ps1 -Scenes menu,warren
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

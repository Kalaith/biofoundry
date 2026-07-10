# Macroquad Toolkit Game Template

This is a working starter crate for new Rust + Macroquad games in this workspace.
It intentionally uses `macroquad-toolkit` heavily so new projects begin with the
same shared patterns as the existing games.

## Toolkit Features Already Wired

- `AssetManager` with a texture manifest at `assets/data/texture_manifest.json`
- `DataRegistry` and embedded JSON loading for data-driven actions
- `save_to_slot_with_version`, `load_from_slot_with_migration`, `delete_slot`, and `get_save_slots`
- `NotificationManager` with toolkit toast rendering
- `VirtualUi`, `SurfaceStyle`, `TextStyle`, `GridLayout`, meters, badges, tooltips, and text fitting
- `FlatGrid`, `FogState`, `TilePos`, line-of-sight visibility, and flood-fill reachability
- `Camera2D` with bounds, right-mouse drag, keyboard pan, and zoom limits
- `EventBus<UiAction>` so UI returns intents and game logic applies them
- Rust 2018 module layout using `data.rs`, `state.rs`, and `ui.rs` parent
  files instead of `mod.rs`

The template avoids browser-incompatible filesystem access. Static data is
embedded with `include_str!()`, runtime browser assets go through Macroquad or
toolkit async loaders, and save data uses macroquad-toolkit persistence.
Shared UI math, such as grid layout and mouse selection, is kept in helper
types so rendering and input do not duplicate coordinate calculations.

## Run

```powershell
cargo run --manifest-path template/Cargo.toml
```

## Test

```powershell
cargo test --manifest-path template/Cargo.toml
```

## Rename For A New Game

1. Copy `template/` to your new game folder.
2. Rename the package in `Cargo.toml`.
3. Update `assets/data/game_config.json`.
4. Replace `actions.json` with your game data.
5. Add textures to `assets/data/texture_manifest.json`.
6. Update `index.html` to load the new wasm filename.

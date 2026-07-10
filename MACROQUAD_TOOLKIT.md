# Macroquad Toolkit

A collection of common utilities for Macroquad game development, extracted from multiple games to reduce duplication and provide consistent patterns.

## Features

- **Input utilities**: Mouse hovering, clicking, rectangle collision detection
- **UI rendering**: Buttons (with press/release variants), panels, progress bars
- **Asset management**: Texture loading and caching
- **Camera2D**: Pan and zoom for 2D games
- **Event bus**: Generic event system for decoupled game logic
- **Color palettes**: Consistent dark theme colors
- **Sprite system**: Builder pattern for texture rendering with transformations
- **Screenshot capture**: Env-var-driven headless capture harness for visual verification

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
macroquad-toolkit = { path = "../macroquad-toolkit" }
```

### Quick Start

```rust
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;

#[macroquad::main("My Game")]
async fn main() {
    let mut assets = AssetManager::new();
    assets.load_texture("player", "assets/player.png").await.ok();

    loop {
        clear_background(dark::BACKGROUND);

        // Draw a button
        if button(10.0, 10.0, 100.0, 40.0, "Click Me") {
            println!("Button clicked!");
        }

        next_frame().await;
    }
}
```

## Modules

### Input (`input` module)

```rust
use macroquad_toolkit::input::*;

// Check if mouse is over a rectangle
if is_hovered(x, y, w, h) {
    // ...
}

// Check if rectangle was clicked (released)
if was_clicked(x, y, w, h) {
    // ...
}

// Check if rectangle was pressed (down)
if was_pressed(x, y, w, h) {
    // ...
}

// Capture input state
let input = InputState::capture();
if input.left_click {
    // ...
}
```

### UI (`ui` module)

```rust
use macroquad_toolkit::ui::*;

// Simple button (triggers on release)
if button(x, y, w, h, "Click") {
    // Button was clicked
}

// Button with custom style
let style = ButtonStyle::default_dark();
if button_styled(x, y, w, h, "Custom", &style) {
    // ...
}

// Button that triggers on press (instead of release)
if button_on_press(x, y, w, h, "Press", &style) {
    // Triggers immediately when mouse down
}

// Panel with title
panel(x, y, w, h, Some("Title"));

// Progress bar
progress_bar(x, y, w, h, current, max, dark::POSITIVE);
```

### Assets (`assets` module)

```rust
use macroquad_toolkit::assets::AssetManager;

let mut assets = AssetManager::new();

// Load single texture
assets.load_texture("player", "assets/player.png").await.ok();

// Get texture
if let Some(tex) = assets.get_texture("player") {
    draw_texture(tex, x, y, WHITE);
}
```

### Camera (`camera` module)

```rust
use macroquad_toolkit::camera::Camera2D;

let mut camera = Camera2D::new(vec2(0.0, 0.0), 1.0);

// In game loop
camera.update(get_frame_time(), false);

// Convert coordinates
let world_pos = camera.screen_to_world(mouse_position().into());
let screen_pos = camera.world_to_screen(world_pos);
```

### Events (`events` module)

```rust
use macroquad_toolkit::events::EventBus;

enum GameEvent {
    PlayerDied,
    EnemySpawned,
}

let mut events = EventBus::new();
events.push(GameEvent::PlayerDied);

// Process events
for event in events.drain() {
    match event {
        GameEvent::PlayerDied => { /* ... */ }
        GameEvent::EnemySpawned => { /* ... */ }
    }
}
```

### Colors (`colors` module)

```rust
use macroquad_toolkit::colors::dark;

clear_background(dark::BACKGROUND);
draw_rectangle(x, y, w, h, dark::PANEL);
draw_text("Hello", x, y, 20.0, dark::TEXT);
```

Available colors:
- `BACKGROUND`, `PANEL`, `PANEL_HEADER`
- `TEXT`, `TEXT_BRIGHT`, `TEXT_DIM`
- `ACCENT`, `POSITIVE`, `WARNING`, `NEGATIVE`
- `HOVERED`

### RNG (`rng` module)

Two layers: convenience wrappers around `macroquad::rand` (WebGL-safe, shared
generator) and `SeededRng`, a deterministic xorshift64* generator for
gameplay. Prefer `SeededRng` for anything that affects simulation outcomes —
keep it state-owned so runs are reproducible (`CODE_STANDARDS.md`), and use
the shared-generator helpers only for cosmetic randomness. Do **not** write a
project-local RNG; games that need one re-export this
(e.g. `pub use macroquad_toolkit::rng::SeededRng as Rng;`).

```rust
use macroquad_toolkit::rng::{self, SeededRng};

// Deterministic, state-owned gameplay RNG. Serde-serializable so mid-run
// state can live in saves.
let mut rng = SeededRng::new(world_seed);
let roll = rng.next_u64();          // raw 64 bits
let t = rng.next_f32();             // [0, 1)
let speed = rng.range_f32(0.5, 2.0); // [low, high)
let index = rng.below(items.len()); // [0, n); 0 when n == 0
if rng.chance(0.25) { /* 25% */ }
let picked = rng.choose(&items);    // Option<&T>

// Shared-generator helpers (cosmetic randomness only).
rng::srand(seed);
let v = rng::gen_range(0, 10);
let flip = rng::chance(0.5);
rng::shuffle(&mut deck);
let one = rng::choose(&palette);
```

### Sprite (`sprite` module)

```rust
use macroquad_toolkit::sprite::Sprite;

let sprite = Sprite::new()
    .with_texture(texture)
    .at(100.0, 100.0)
    .scaled(2.0, 2.0)
    .rotated(0.5)
    .colored(RED);

sprite.draw();
```

### Capture (`capture` module)

Headless screenshot harness: when a `PREFIX_CAPTURE_PATH` env var is set, the
game boots into a chosen scene, simulates a fixed number of frames at a fixed
timestep, writes a PNG, and exits. This makes UI changes visually verifiable
from a script (or by an AI agent reading the PNG back) with no interactive
input. Full walkthrough and gotchas: `docs/screenshot_capture_harness_guide.md`.

```rust
use macroquad_toolkit::capture;

fn window_conf() -> Conf {
    // Reads MYGAME_WINDOW_WIDTH/HEIGHT overrides; disables high_dpi while
    // capturing so screenshots are pixel-aligned with the logical layout.
    capture::capture_window_conf("MYGAME", "My Game", 1280, 720)
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut game = Game::new().await;

    if let Some(config) = capture::CaptureConfig::from_env("MYGAME") {
        game.begin_capture_scene(&config.scene); // your scene-seeding method
        capture::run_capture(&config, |dt| {
            game.update(dt);
            game.draw();
        })
        .await;
        return;
    }

    loop { /* normal interactive loop */ }
}
```

The only per-game code is `begin_capture_scene(&str)` — a method that puts the
session into a named scene (e.g. `"gameplay"`, `"map"`, `"loadout"`) so the
capture starts in the state you want to photograph.

Env vars (replace `MYGAME` with your per-game prefix):

- `MYGAME_CAPTURE_PATH` — output PNG path; presence enables capture mode
- `MYGAME_CAPTURE_SCENE` — scene name (default `gameplay`)
- `MYGAME_CAPTURE_FRAMES` — frames to simulate before capture (default 150)
- `MYGAME_WINDOW_WIDTH` / `MYGAME_WINDOW_HEIGHT` — window size override

All env access is stubbed out on `wasm32`, so web builds are unaffected.

A shared wrapper script (`macroquad-toolkit/scripts/capture_ui.ps1`) builds the
game, runs one capture per scene, and sanity-checks each PNG. It derives the
package name, exe path, and env prefix from `cargo metadata`, so from a game
directory it needs no arguments:

```powershell
& ..\macroquad-toolkit\scripts\capture_ui.ps1 -Scenes gameplay,map
& ..\macroquad-toolkit\scripts\capture_ui.ps1 -Scenes gameplay -SkipBuild
```

Pass `-Prefix` if the game's env-var prefix differs from its package name
(e.g. `carriage_run` uses `CARRIAGE`). See `carriage_run` for a reference
integration, including a thin per-game `scripts/capture_ui.ps1` wrapper.

## Button Click Semantics

The toolkit provides two button variants to handle different click behaviors:

- **`button()` and `button_on_release()`**: Fire when mouse button is **released** over the button. This is the safer default as it prevents accidental double-clicks and allows users to move the mouse away to cancel.

- **`button_on_press()`**: Fires when mouse button is **pressed down** over the button. Use this for instant feedback scenarios.

## License

This toolkit is extracted from game projects and shared for reuse across multiple games.

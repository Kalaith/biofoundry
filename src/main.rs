//! Biofoundry — a factory/colony hybrid where every piece of automation is
//! a living creature and food is the power grid.

use macroquad::prelude::*;
use macroquad_toolkit::capture;

mod data;
mod game;
mod simulation;
mod state;
mod ui;

use game::Game;

fn window_conf() -> Conf {
    capture::capture_window_conf(
        "BIOFOUNDRY",
        "Biofoundry",
        ui::LOGICAL_WIDTH as i32,
        ui::LOGICAL_HEIGHT as i32,
    )
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut game = Game::new().await;

    // Screenshot harness: when BIOFOUNDRY_CAPTURE_PATH is set, seed the named
    // scene ("menu" or "warren"), render deterministic frames, write a PNG,
    // and exit. Stubbed out on wasm32.
    if let Some(config) = capture::CaptureConfig::from_env("BIOFOUNDRY") {
        game.begin_capture_scene(&config.scene);
        capture::run_capture(&config, |dt| {
            game.update(dt);
            game.draw();
        })
        .await;
        return;
    }

    loop {
        let dt = get_frame_time().min(0.1);
        game.update(dt);
        game.draw();
        next_frame().await;
    }
}

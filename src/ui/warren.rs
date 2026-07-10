//! Warren world rendering: tiles, buildings, and creatures in camera
//! space. Pure view — reads the session and draws.

use crate::state::creatures::{Creature, Job};
use crate::state::world::Tile;
use crate::state::GameSession;
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;

/// Draw the tile map, buildings, and creatures in world coordinates.
/// Call inside `camera.begin()`.
pub fn draw_world(session: &GameSession, tile_size: f32) {
    draw_tiles(session, tile_size);
    draw_buildings(session, tile_size);
    for creature in &session.creatures {
        draw_creature(creature, tile_size);
    }
}

fn draw_tiles(session: &GameSession, ts: f32) {
    for (pos, tile) in session.world.tiles.iter_with_pos() {
        let x = pos.x as f32 * ts;
        let y = pos.y as f32 * ts;
        draw_rectangle(x, y, ts, ts, tile_color(*tile));

        match tile {
            Tile::MushroomPatch => {
                let grown = session
                    .patch_regrow
                    .get(&pos)
                    .is_none_or(|regrow| *regrow <= 0.0);
                let color = if grown {
                    Color::new(0.85, 0.75, 0.55, 1.0)
                } else {
                    Color::new(0.45, 0.40, 0.32, 1.0)
                };
                draw_circle(x + ts * 0.5, y + ts * 0.5, ts * 0.2, color);
            }
            Tile::OreVein => {
                let inset = ts * 0.28;
                draw_rectangle(
                    x + inset,
                    y + inset,
                    ts - inset * 2.0,
                    ts - inset * 2.0,
                    Color::new(0.75, 0.62, 0.35, 1.0),
                );
            }
            _ => {}
        }
    }
}

fn tile_color(tile: Tile) -> Color {
    match tile {
        Tile::Rock => Color::new(0.13, 0.12, 0.14, 1.0),
        Tile::Floor => Color::new(0.24, 0.20, 0.17, 1.0),
        Tile::Water => Color::new(0.16, 0.30, 0.42, 1.0),
        Tile::MushroomPatch => Color::new(0.24, 0.20, 0.17, 1.0),
        Tile::OreVein => Color::new(0.20, 0.17, 0.16, 1.0),
    }
}

fn draw_buildings(session: &GameSession, ts: f32) {
    let b = &session.buildings;

    // Mushroom Farm: green plot with sprout dots.
    let (fx, fy) = (b.farm.x as f32 * ts, b.farm.y as f32 * ts);
    draw_rectangle(
        fx + 2.0,
        fy + 2.0,
        ts - 4.0,
        ts - 4.0,
        Color::new(0.16, 0.30, 0.16, 1.0),
    );
    let stock = session.economy.farm_mushrooms;
    for i in 0..3 {
        let filled = stock >= (i as f32 + 1.0) * 4.0;
        let color = if filled {
            Color::new(0.80, 0.72, 0.50, 1.0)
        } else {
            Color::new(0.30, 0.42, 0.28, 1.0)
        };
        draw_circle(
            fx + ts * (0.25 + 0.25 * i as f32),
            fy + ts * 0.5,
            ts * 0.11,
            color,
        );
    }

    // Cook Pot: dark cauldron ring.
    let (px, py) = (
        b.cook_pot.x as f32 * ts + ts * 0.5,
        b.cook_pot.y as f32 * ts + ts * 0.5,
    );
    draw_circle(px, py, ts * 0.34, Color::new(0.16, 0.12, 0.10, 1.0));
    draw_circle_lines(px, py, ts * 0.34, 2.0, Color::new(0.85, 0.55, 0.25, 1.0));
    if session.economy.pot_mushrooms > 0 {
        draw_circle(px, py, ts * 0.14, Color::new(0.85, 0.75, 0.55, 1.0));
    }

    // Stockpile: slab outline at spawn.
    let (sx, sy) = (b.stockpile.x as f32 * ts, b.stockpile.y as f32 * ts);
    draw_rectangle_lines(
        sx + 3.0,
        sy + 3.0,
        ts - 6.0,
        ts - 6.0,
        2.0,
        Color::new(0.65, 0.66, 0.70, 0.9),
    );
}

fn draw_creature(creature: &Creature, ts: f32) {
    let x = creature.x * ts;
    let y = creature.y * ts;
    let is_beetle = creature.species == "beetle";
    let radius = if is_beetle { ts * 0.34 } else { ts * 0.24 };

    draw_circle(x, y, radius, job_color(creature.job, is_beetle));

    // Hunger telegraph: amber ring when hungry, red when starving.
    if creature.satiation <= 0.33 {
        draw_circle_lines(x, y, radius + 2.0, 2.0, dark::NEGATIVE);
    } else if creature.satiation <= 0.66 {
        draw_circle_lines(x, y, radius + 2.0, 2.0, dark::WARNING);
    }

    // Carried goods: a small dot on the back.
    if creature.carrying > 0 {
        draw_circle(
            x,
            y - radius * 0.9,
            ts * 0.09,
            Color::new(0.9, 0.85, 0.7, 1.0),
        );
    }
}

fn job_color(job: Job, is_beetle: bool) -> Color {
    if is_beetle {
        return Color::new(0.62, 0.40, 0.75, 1.0);
    }
    match job {
        Job::Miner => Color::new(0.45, 0.62, 0.85, 1.0),
        Job::Carrier => Color::new(0.85, 0.75, 0.38, 1.0),
        Job::Cook => Color::new(0.88, 0.52, 0.28, 1.0),
        Job::Idle => Color::new(0.55, 0.55, 0.58, 1.0),
    }
}

//! Warren world rendering: tiles, buildings, build sites, dig marks, and
//! creatures in camera space. Pure view — reads the session and draws.

use crate::state::creatures::{Creature, Job};
use crate::state::structures::Building;
use crate::state::world::Tile;
use crate::state::GameSession;
use crate::ui::UiMode;
use macroquad::prelude::*;
use macroquad_toolkit::grid::TilePos;
use macroquad_toolkit::prelude::*;

/// Draw the world in camera space. `hover` is the tile under the cursor
/// when the pointer is free (used for build/dig ghosts).
pub fn draw_world(session: &GameSession, tile_size: f32, mode: &UiMode, hover: Option<TilePos>) {
    draw_tiles(session, tile_size);
    draw_dig_marks(session, tile_size);
    for building in &session.buildings {
        draw_building(session, building, tile_size);
    }
    draw_build_sites(session, tile_size);
    for creature in &session.creatures {
        draw_creature(creature, tile_size);
    }
    draw_tool_ghost(session, tile_size, mode, hover);
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

fn draw_dig_marks(session: &GameSession, ts: f32) {
    for mark in &session.dig_marks {
        let x = mark.x as f32 * ts;
        let y = mark.y as f32 * ts;
        draw_rectangle(x, y, ts, ts, Color::new(0.95, 0.75, 0.35, 0.22));
        draw_rectangle_lines(
            x + 2.0,
            y + 2.0,
            ts - 4.0,
            ts - 4.0,
            2.0,
            Color::new(0.95, 0.75, 0.35, 0.8),
        );
    }
}

fn draw_building(session: &GameSession, building: &Building, ts: f32) {
    let (x, y) = (building.pos.x as f32 * ts, building.pos.y as f32 * ts);
    match building.kind.as_str() {
        "farm" => {
            draw_rectangle(
                x + 2.0,
                y + 2.0,
                ts - 4.0,
                ts - 4.0,
                Color::new(0.16, 0.30, 0.16, 1.0),
            );
            for i in 0..3 {
                let filled = building.stock >= (i as f32 + 1.0) * 4.0;
                let color = if filled {
                    Color::new(0.80, 0.72, 0.50, 1.0)
                } else {
                    Color::new(0.30, 0.42, 0.28, 1.0)
                };
                draw_circle(
                    x + ts * (0.25 + 0.25 * i as f32),
                    y + ts * 0.5,
                    ts * 0.11,
                    color,
                );
            }
        }
        "cook_pot" => {
            let (cx, cy) = (x + ts * 0.5, y + ts * 0.5);
            draw_circle(cx, cy, ts * 0.34, Color::new(0.16, 0.12, 0.10, 1.0));
            draw_circle_lines(cx, cy, ts * 0.34, 2.0, Color::new(0.85, 0.55, 0.25, 1.0));
            if building.stock >= 1.0 {
                draw_circle(cx, cy, ts * 0.14, Color::new(0.85, 0.75, 0.55, 1.0));
            }
        }
        "stockpile" => {
            draw_rectangle_lines(
                x + 3.0,
                y + 3.0,
                ts - 6.0,
                ts - 6.0,
                2.0,
                Color::new(0.65, 0.66, 0.70, 0.9),
            );
            let dots = (session.economy.ore_stock.min(9) as usize).div_euclid(3);
            for i in 0..=dots {
                if session.economy.ore_stock > 0 {
                    draw_circle(
                        x + ts * (0.3 + 0.2 * i as f32),
                        y + ts * 0.65,
                        ts * 0.08,
                        Color::new(0.75, 0.62, 0.35, 1.0),
                    );
                }
            }
        }
        _ => {}
    }
}

fn draw_build_sites(session: &GameSession, ts: f32) {
    for site in &session.build_sites {
        let x = site.pos.x as f32 * ts;
        let y = site.pos.y as f32 * ts;
        draw_rectangle(x, y, ts, ts, Color::new(0.55, 0.75, 0.95, 0.15));
        draw_rectangle_lines(
            x + 2.0,
            y + 2.0,
            ts - 4.0,
            ts - 4.0,
            2.0,
            Color::new(0.55, 0.75, 0.95, 0.9),
        );
        // Delivery progress: a fill bar along the bottom edge.
        let frac = if site.ore_needed == 0 {
            1.0
        } else {
            site.ore_delivered as f32 / site.ore_needed as f32
        };
        draw_rectangle(
            x + 3.0,
            y + ts - 7.0,
            (ts - 6.0) * frac.clamp(0.0, 1.0),
            4.0,
            Color::new(0.55, 0.85, 0.55, 0.95),
        );
    }
}

fn draw_tool_ghost(session: &GameSession, ts: f32, mode: &UiMode, hover: Option<TilePos>) {
    let Some(tile) = hover else {
        return;
    };
    let x = tile.x as f32 * ts;
    let y = tile.y as f32 * ts;
    match mode {
        UiMode::Build(_) => {
            let ok = session.can_place_building(tile);
            let color = if ok {
                Color::new(0.45, 0.9, 0.5, 0.9)
            } else {
                Color::new(0.9, 0.35, 0.3, 0.9)
            };
            draw_rectangle_lines(x + 1.0, y + 1.0, ts - 2.0, ts - 2.0, 3.0, color);
        }
        UiMode::Dig => {
            let diggable = session
                .world
                .tiles
                .get(tile)
                .is_some_and(|t| matches!(t, Tile::Rock | Tile::OreVein));
            let color = if diggable {
                Color::new(0.95, 0.75, 0.35, 0.9)
            } else {
                Color::new(0.6, 0.6, 0.6, 0.6)
            };
            draw_rectangle_lines(x + 1.0, y + 1.0, ts - 2.0, ts - 2.0, 3.0, color);
        }
        UiMode::Inspect => {}
    }
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
    if let Some((good, _)) = creature.carrying {
        let color = match good {
            crate::state::creatures::Good::Mushroom => Color::new(0.9, 0.85, 0.7, 1.0),
            crate::state::creatures::Good::Ore => Color::new(0.75, 0.62, 0.35, 1.0),
        };
        draw_circle(x, y - radius * 0.9, ts * 0.09, color);
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

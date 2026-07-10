//! Warren (gameplay) rendering: world tiles in camera space plus a HUD in
//! screen space. Pure view — reads the session, returns intents.

use crate::simulation;
use crate::state::world::Tile;
use crate::state::GameSession;
use crate::ui::{UiAction, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::draw_ui_text_ex;

/// Draw the tile map in world coordinates. Call inside `camera.begin()`.
pub fn draw_world(session: &GameSession, tile_size: f32) {
    for (pos, tile) in session.world.tiles.iter_with_pos() {
        let x = pos.x as f32 * tile_size;
        let y = pos.y as f32 * tile_size;
        draw_rectangle(x, y, tile_size, tile_size, tile_color(*tile));

        // Subtle inset marks resource tiles readable at a glance.
        match tile {
            Tile::MushroomPatch => {
                let inset = tile_size * 0.3;
                draw_circle(
                    x + tile_size * 0.5,
                    y + tile_size * 0.5,
                    tile_size * 0.5 - inset,
                    Color::new(0.85, 0.75, 0.55, 1.0),
                );
            }
            Tile::OreVein => {
                let inset = tile_size * 0.28;
                draw_rectangle(
                    x + inset,
                    y + inset,
                    tile_size - inset * 2.0,
                    tile_size - inset * 2.0,
                    Color::new(0.75, 0.62, 0.35, 1.0),
                );
            }
            _ => {}
        }
    }

    // Spawn marker: where the warren begins.
    let (sx, sy) = session.world.spawn.to_f32();
    draw_circle_lines(
        sx * tile_size + tile_size * 0.5,
        sy * tile_size + tile_size * 0.5,
        tile_size * 0.6,
        2.0,
        dark::ACCENT,
    );
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

/// Draw the screen-space HUD. Call after `set_default_camera()`.
pub fn draw_hud(session: &GameSession, ui: &VirtualUi) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let mouse = ui.mouse_position();

    let bar = Rect::new(12.0, 12.0, LOGICAL_WIDTH - 24.0, 48.0);
    draw_surface(
        bar,
        &SurfaceStyle::new(Color::new(0.07, 0.08, 0.10, 0.94))
            .with_border(1.0, Color::new(0.38, 0.45, 0.58, 0.55)),
    );

    draw_ui_text_ex(
        "Biofoundry — Warren",
        bar.x + 16.0,
        bar.y + 31.0,
        TextStyle::new(22.0, dark::TEXT_BRIGHT).params(),
    );

    let seconds = simulation::sim_seconds(session);
    draw_ui_text_ex(
        &format!("Sim {:02}:{:04.1}", (seconds / 60.0) as u32, seconds % 60.0),
        bar.x + 300.0,
        bar.y + 31.0,
        TextStyle::new(18.0, dark::TEXT).params(),
    );

    draw_ui_text_ex(
        "Right-drag / WASD pan · wheel zoom · Esc menu",
        bar.x + 480.0,
        bar.y + 31.0,
        TextStyle::new(16.0, dark::TEXT_DIM).params(),
    );

    let btn = Rect::new(bar.right() - 96.0, bar.y + 8.0, 84.0, 32.0);
    let hovered = btn.contains_point(mouse);
    draw_surface(
        btn,
        &SurfaceStyle::new(if hovered {
            Color::new(0.20, 0.22, 0.28, 1.0)
        } else {
            Color::new(0.13, 0.145, 0.18, 1.0)
        })
        .with_border(1.0, Color::new(0.5, 0.55, 0.65, 0.5)),
    );
    draw_text_centered_in_box_ex(
        "Menu",
        btn.x,
        btn.y,
        btn.w,
        btn.h,
        TextStyle::new(17.0, dark::TEXT),
    );
    if hovered && is_mouse_button_released(MouseButton::Left) {
        actions.push(UiAction::BackToMenu);
    }

    actions
}

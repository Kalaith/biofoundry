//! Screen-space HUD: the calorie balance meter (the game's "power UI"),
//! job assignment panel, build tools, and victory overlay. Pure view —
//! returns intents and whether the pointer is over HUD chrome.
//!
//! This file is the layout: it owns the panel rects, calls each piece
//! (`panels`, `inspect`, `overlays`), and folds their output into one
//! `HudFrame`.

mod inspect;
mod overlays;
mod panels;
mod widgets;

use crate::data::GameData;
use crate::simulation;
use crate::state::GameSession;
use crate::ui::{HudFrame, UiAction, UiMode, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::grid::TilePos;
use macroquad_toolkit::prelude::*;

const PANEL_W: f32 = 252.0;

pub fn draw(
    session: &GameSession,
    data: &GameData,
    ui: &VirtualUi,
    mode: &UiMode,
    selected: Option<TilePos>,
) -> HudFrame {
    let mut actions = Vec::new();
    let mouse = ui.mouse_position();

    let top_bar = Rect::new(12.0, 12.0, LOGICAL_WIDTH - 24.0, 48.0);
    let food_panel = Rect::new(12.0, 66.0, PANEL_W, 184.0);
    let jobs_panel = Rect::new(12.0, 256.0, PANEL_W, 262.0);
    let tools_panel = Rect::new(12.0, 522.0, PANEL_W, 190.0);

    panels::draw_top_bar(session, top_bar, mouse, &mut actions);
    panels::draw_food_grid_panel(session, data, food_panel);
    panels::draw_jobs_panel(session, data, jobs_panel, mouse, &mut actions);
    panels::draw_tools_panel(session, data, tools_panel, mode, mouse, &mut actions);
    let tutorial_panel = panels::draw_tutorial_panel(session, data, mouse, &mut actions);
    let inspect_panel = selected
        .and_then(|pos| inspect::draw_inspect_panel(session, data, pos, mouse, &mut actions));

    // A status-icon legend, shown only while some node is stalled — it
    // teaches the in-world badges exactly when they matter.
    if session
        .buildings
        .iter()
        .any(|b| crate::ui::legibility::building_status(session, data, b).is_some())
    {
        overlays::draw_status_legend();
    }

    let victory_up = session.won && !session.victory_shown;
    let factory_up = session.factory_complete && !session.factory_shown;
    let worm_up = session.worm_awake && !session.worm_shown;
    if worm_up {
        overlays::draw_goal_overlay(
            "The Colossal Worm Awakens",
            &format!(
                "Fed on {:.0} offerings, the great worm rises from the deep and coils around the warren that raised it.\n\nThe campaign is complete in {:.0} minutes. The warren — and its worm — play on.",
                session.worm_fed,
                simulation::sim_seconds(session) / 60.0
            ),
            UiAction::DismissWorm,
            mouse,
            &mut actions,
        );
    } else if factory_up {
        overlays::draw_goal_overlay(
            "Factory Complete",
            &format!(
                "The Biofoundry roars: {} ingots forged by hammer and living furnace in {:.0} minutes.\n\nEvery belt breathes. Keep playing, or return to the menu.",
                session.economy.ingots_forged,
                simulation::sim_seconds(session) / 60.0
            ),
            UiAction::DismissFactory,
            mouse,
            &mut actions,
        );
    } else if victory_up {
        overlays::draw_goal_overlay(
            "Victory",
            &format!(
                "The warren thrives: a 100-food surplus and {} ore delivered in {:.0} minutes.\n\nNext: place a Blacksmith and hammer ore into {} ingots (a Smelter Den + salamander forges them in bulk).",
                session.economy.ore_delivered_total,
                simulation::sim_seconds(session) / 60.0,
                data.balance.win2_ingots
            ),
            UiAction::DismissVictory,
            mouse,
            &mut actions,
        );
    }

    let pointer_over_ui = victory_up
        || factory_up
        || worm_up
        || tutorial_panel.is_some_and(|r| r.contains_point(mouse))
        || inspect_panel.is_some_and(|r| r.contains_point(mouse))
        || [top_bar, food_panel, jobs_panel, tools_panel]
            .iter()
            .any(|r| r.contains_point(mouse));

    HudFrame {
        actions,
        pointer_over_ui,
    }
}

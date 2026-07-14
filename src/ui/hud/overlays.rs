//! Full-screen goal overlays (victory / factory / worm) and the in-world
//! status-badge legend.

use crate::ui::hud::widgets::{hud_button, panel_style};
use crate::ui::{UiAction, LOGICAL_HEIGHT, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::draw_ui_text_ex;

pub(super) fn draw_goal_overlay(
    title: &str,
    body: &str,
    dismiss: UiAction,
    mouse: Vec2,
    actions: &mut Vec<UiAction>,
) {
    draw_rectangle(
        0.0,
        0.0,
        LOGICAL_WIDTH,
        LOGICAL_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.55),
    );
    let panel = Rect::new(LOGICAL_WIDTH * 0.5 - 240.0, 200.0, 480.0, 250.0);
    draw_surface_with_title(
        panel,
        Some(title),
        &panel_style(),
        TextStyle::new(20.0, dark::TEXT_BRIGHT),
    );

    draw_text_block(
        body,
        panel.x + 20.0,
        panel.y + 60.0,
        panel.w - 40.0,
        110.0,
        17.0,
        5.0,
        dark::TEXT,
    );

    if hud_button(
        Rect::new(panel.x + 70.0, panel.bottom() - 56.0, 160.0, 38.0),
        "Keep Playing",
        true,
        mouse,
    ) {
        actions.push(dismiss);
    }
    if hud_button(
        Rect::new(panel.x + 250.0, panel.bottom() - 56.0, 160.0, 38.0),
        "Menu",
        true,
        mouse,
    ) {
        actions.push(UiAction::BackToMenu);
    }
}

/// A one-line legend for the in-world status badges, in a thin strip along
/// the bottom of the world view (shown only while a node is stalled).
pub(super) fn draw_status_legend() {
    use crate::ui::legibility::BuildingStatus as St;
    let strip = Rect::new(280.0, LOGICAL_HEIGHT - 30.0, LOGICAL_WIDTH - 292.0, 24.0);
    draw_surface(
        strip,
        &SurfaceStyle::new(Color::new(0.06, 0.07, 0.09, 0.88))
            .with_border(1.0, Color::new(0.38, 0.45, 0.58, 0.4)),
    );
    let items = [
        (St::NoWorker, Color::new(0.95, 0.85, 0.30, 1.0)),
        (St::InputStarved, Color::new(0.95, 0.55, 0.20, 1.0)),
        (St::OutputFull, Color::new(0.92, 0.32, 0.26, 1.0)),
        (St::AwaitingHaul, Color::new(0.40, 0.80, 0.92, 1.0)),
        (St::Exhausted, Color::new(0.60, 0.60, 0.66, 1.0)),
    ];
    let mut lx = strip.x + 12.0;
    let cy = strip.y + strip.h * 0.5;
    for (status, color) in items {
        draw_circle(lx, cy, 5.0, color);
        let label = status.label();
        draw_ui_text_ex(
            label,
            lx + 12.0,
            cy + 5.0,
            TextStyle::new(13.0, dark::TEXT).params(),
        );
        lx += 20.0 + label.len() as f32 * 8.0;
    }
}

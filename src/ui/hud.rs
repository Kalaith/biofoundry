//! Screen-space HUD: the calorie balance meter (the game's "power UI"),
//! job assignment panel, and victory overlay. Pure view — returns intents.

use crate::data::GameData;
use crate::simulation::{self, food};
use crate::state::creatures::Job;
use crate::state::GameSession;
use crate::ui::{UiAction, LOGICAL_HEIGHT, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::draw_ui_text_ex;

pub fn draw(session: &GameSession, data: &GameData, ui: &VirtualUi) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let mouse = ui.mouse_position();

    draw_top_bar(session, mouse, &mut actions);
    draw_food_grid_panel(session, data);
    draw_jobs_panel(session, data, mouse, &mut actions);
    if session.won && !session.victory_shown {
        draw_victory_overlay(session, mouse, &mut actions);
    }

    actions
}

fn draw_top_bar(session: &GameSession, mouse: Vec2, actions: &mut Vec<UiAction>) {
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
        &format!("{:02}:{:02}", (seconds / 60.0) as u32, seconds as u32 % 60),
        bar.x + 290.0,
        bar.y + 31.0,
        TextStyle::new(18.0, dark::TEXT).params(),
    );

    if session.economy.food <= 0.0 {
        draw_ui_text_ex(
            "FAMINE — workers are slowing down",
            bar.x + 380.0,
            bar.y + 31.0,
            TextStyle::new(18.0, dark::NEGATIVE).params(),
        );
    } else {
        draw_ui_text_ex(
            "Right-drag / WASD pan · wheel zoom · Esc menu",
            bar.x + 380.0,
            bar.y + 31.0,
            TextStyle::new(15.0, dark::TEXT_DIM).params(),
        );
    }

    if hud_button(
        Rect::new(bar.right() - 96.0, bar.y + 8.0, 84.0, 32.0),
        "Menu",
        true,
        mouse,
    ) {
        actions.push(UiAction::BackToMenu);
    }
}

/// The calorie balance meter — production, consumption, stockpile, and
/// time-to-empty, exactly like a power graph.
fn draw_food_grid_panel(session: &GameSession, data: &GameData) {
    let panel = Rect::new(12.0, 72.0, 252.0, 214.0);
    draw_surface_with_title(
        panel,
        Some("Food Grid"),
        &panel_style(),
        TextStyle::new(17.0, dark::TEXT),
    );

    let production = session.economy.production_ema_per_min.max(0.0);
    let consumption = food::consumption_per_min(session, data);
    let net = production - consumption;
    let x = panel.x + 14.0;
    let mut y = panel.y + 56.0;

    draw_ui_text_ex(
        &format!("Production  +{production:.1}/min"),
        x,
        y,
        TextStyle::new(16.0, dark::POSITIVE).params(),
    );
    y += 24.0;
    draw_ui_text_ex(
        &format!("Upkeep      -{consumption:.1}/min"),
        x,
        y,
        TextStyle::new(16.0, dark::NEGATIVE).params(),
    );
    y += 24.0;
    let net_color = if net >= 0.0 {
        dark::POSITIVE
    } else {
        dark::NEGATIVE
    };
    draw_ui_text_ex(
        &format!("Net         {net:+.1}/min"),
        x,
        y,
        TextStyle::new(16.0, net_color).params(),
    );
    y += 18.0;

    meter(
        Rect::new(x, y, panel.w - 28.0, 20.0),
        session.economy.food,
        data.balance.win_food_surplus,
        if session.economy.food > 15.0 {
            dark::POSITIVE
        } else {
            dark::NEGATIVE
        },
        Some(&format!(
            "Food {:.0}/{:.0}",
            session.economy.food, data.balance.win_food_surplus
        )),
    );
    y += 36.0;

    match food::time_to_empty_sec(session.economy.food, production, consumption) {
        Some(secs) if session.economy.food > 0.0 => {
            draw_ui_text_ex(
                &format!("Empty in {:.0}s", secs),
                x,
                y,
                TextStyle::new(15.0, dark::WARNING).params(),
            );
        }
        _ => {}
    }
    y += 22.0;

    draw_ui_text_ex(
        &format!(
            "Farm {:.0} shrooms · Pot {}",
            session.economy.farm_mushrooms, session.economy.pot_mushrooms
        ),
        x,
        y,
        TextStyle::new(14.0, dark::TEXT_DIM).params(),
    );
    y += 22.0;
    draw_ui_text_ex(
        &format!(
            "Ore delivered {}/{}",
            session.economy.ore_delivered, data.balance.win_ore_delivered
        ),
        x,
        y,
        TextStyle::new(15.0, dark::TEXT).params(),
    );
}

fn draw_jobs_panel(
    session: &GameSession,
    data: &GameData,
    mouse: Vec2,
    actions: &mut Vec<UiAction>,
) {
    let panel = Rect::new(12.0, 296.0, 252.0, 236.0);
    draw_surface_with_title(
        panel,
        Some("Jobs"),
        &panel_style(),
        TextStyle::new(17.0, dark::TEXT),
    );

    let idle = session.job_count(Job::Idle);
    let x = panel.x + 14.0;
    let mut y = panel.y + 50.0;

    for job in [Job::Miner, Job::Carrier, Job::Cook] {
        let count = session.job_count(job);
        draw_ui_text_ex(
            &format!("{:<8}{count}", job.label()),
            x,
            y + 20.0,
            TextStyle::new(17.0, dark::TEXT).params(),
        );
        if hud_button(Rect::new(x + 130.0, y, 34.0, 28.0), "-", count > 0, mouse) {
            actions.push(UiAction::Unassign(job));
        }
        if hud_button(Rect::new(x + 172.0, y, 34.0, 28.0), "+", idle > 0, mouse) {
            actions.push(UiAction::Assign(job));
        }
        y += 38.0;
    }

    draw_ui_text_ex(
        &format!("Idle    {idle}"),
        x,
        y + 20.0,
        TextStyle::new(17.0, dark::TEXT_DIM).params(),
    );
    y += 42.0;

    let beetles = session
        .creatures
        .iter()
        .filter(|c| c.species == "beetle")
        .count();
    let cost = data.balance.beetle_ore_cost;
    let affordable = session.economy.ore_delivered >= cost;
    if hud_button(
        Rect::new(x, y, panel.w - 28.0, 34.0),
        &format!("Attract Beetle ({cost} ore) · have {beetles}"),
        affordable,
        mouse,
    ) {
        actions.push(UiAction::AttractBeetle);
    }
}

fn draw_victory_overlay(session: &GameSession, mouse: Vec2, actions: &mut Vec<UiAction>) {
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
        Some("Victory"),
        &panel_style(),
        TextStyle::new(20.0, dark::TEXT_BRIGHT),
    );

    let minutes = simulation::sim_seconds(session) / 60.0;
    draw_text_block(
        &format!(
            "The warren thrives: a 100-food surplus and {} ore delivered in {:.0} minutes.\n\nThe hunger grid held. Keep playing, or return to the menu.",
            session.economy.ore_delivered, minutes
        ),
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
        actions.push(UiAction::DismissVictory);
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

fn panel_style() -> SurfaceStyle {
    SurfaceStyle::new(Color::new(0.07, 0.08, 0.10, 0.94))
        .with_border(1.0, Color::new(0.38, 0.45, 0.58, 0.55))
        .with_header(34.0, Color::new(0.09, 0.105, 0.13, 1.0))
        .with_header_divider(1.0, Color::new(0.38, 0.45, 0.58, 0.4))
}

fn hud_button(rect: Rect, text: &str, enabled: bool, mouse: Vec2) -> bool {
    let hovered = enabled && rect.contains_point(mouse);
    let fill = if !enabled {
        Color::new(0.10, 0.11, 0.13, 1.0)
    } else if hovered {
        Color::new(0.20, 0.22, 0.28, 1.0)
    } else {
        Color::new(0.13, 0.145, 0.18, 1.0)
    };
    draw_surface(
        rect,
        &SurfaceStyle::new(fill).with_border(1.0, Color::new(0.5, 0.55, 0.65, 0.5)),
    );
    draw_text_centered_in_box_ex(
        text,
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        TextStyle::new(15.0, if enabled { dark::TEXT } else { dark::TEXT_DIM }),
    );
    hovered && is_mouse_button_released(MouseButton::Left)
}

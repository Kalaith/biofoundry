//! Screen-space HUD: the calorie balance meter (the game's "power UI"),
//! job assignment panel, build tools, and victory overlay. Pure view —
//! returns intents and whether the pointer is over HUD chrome.

use crate::data::GameData;
use crate::simulation::{self, food};
use crate::state::creatures::Job;
use crate::state::GameSession;
use crate::ui::{HudFrame, UiAction, UiMode, LOGICAL_HEIGHT, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::draw_ui_text_ex;

const PANEL_W: f32 = 252.0;

pub fn draw(session: &GameSession, data: &GameData, ui: &VirtualUi, mode: &UiMode) -> HudFrame {
    let mut actions = Vec::new();
    let mouse = ui.mouse_position();

    let top_bar = Rect::new(12.0, 12.0, LOGICAL_WIDTH - 24.0, 48.0);
    let food_panel = Rect::new(12.0, 72.0, PANEL_W, 208.0);
    let jobs_panel = Rect::new(12.0, 290.0, PANEL_W, 224.0);
    let tools_panel = Rect::new(12.0, 524.0, PANEL_W, 184.0);

    draw_top_bar(session, top_bar, mouse, &mut actions);
    draw_food_grid_panel(session, data, food_panel);
    draw_jobs_panel(session, data, jobs_panel, mouse, &mut actions);
    draw_tools_panel(session, data, tools_panel, mode, mouse, &mut actions);

    let victory_up = session.won && !session.victory_shown;
    if victory_up {
        draw_victory_overlay(session, mouse, &mut actions);
    }

    let pointer_over_ui = victory_up
        || [top_bar, food_panel, jobs_panel, tools_panel]
            .iter()
            .any(|r| r.contains_point(mouse));

    HudFrame {
        actions,
        pointer_over_ui,
    }
}

fn draw_top_bar(session: &GameSession, bar: Rect, mouse: Vec2, actions: &mut Vec<UiAction>) {
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
            "Right-drag / WASD pan · wheel zoom · Esc cancel/menu",
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
fn draw_food_grid_panel(session: &GameSession, data: &GameData, panel: Rect) {
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

    if let Some(secs) = food::time_to_empty_sec(session.economy.food, production, consumption) {
        if session.economy.food > 0.0 {
            draw_ui_text_ex(
                &format!("Empty in {secs:.0}s"),
                x,
                y,
                TextStyle::new(15.0, dark::WARNING).params(),
            );
        }
    }
    y += 22.0;

    let farm_stock: f32 = session.buildings_of("farm").map(|b| b.stock).sum();
    let pot_stock: f32 = session.buildings_of("cook_pot").map(|b| b.stock).sum();
    draw_ui_text_ex(
        &format!("Farms {farm_stock:.0} shrooms · Pots {pot_stock:.0}"),
        x,
        y,
        TextStyle::new(14.0, dark::TEXT_DIM).params(),
    );
    y += 22.0;
    draw_ui_text_ex(
        &format!(
            "Ore banked {} · delivered {}/{}",
            session.economy.ore_stock,
            session.economy.ore_delivered_total,
            data.balance.win_ore_delivered
        ),
        x,
        y,
        TextStyle::new(15.0, dark::TEXT).params(),
    );
}

fn draw_jobs_panel(
    session: &GameSession,
    data: &GameData,
    panel: Rect,
    mouse: Vec2,
    actions: &mut Vec<UiAction>,
) {
    draw_surface_with_title(
        panel,
        Some("Jobs"),
        &panel_style(),
        TextStyle::new(17.0, dark::TEXT),
    );

    let idle = session.job_count(Job::Idle);
    let x = panel.x + 14.0;
    let mut y = panel.y + 48.0;

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
        y += 36.0;
    }

    draw_ui_text_ex(
        &format!("Idle    {idle}"),
        x,
        y + 20.0,
        TextStyle::new(17.0, dark::TEXT_DIM).params(),
    );
    y += 40.0;

    let beetles = session
        .creatures
        .iter()
        .filter(|c| c.species == "beetle")
        .count();
    let cost = data.balance.beetle_ore_cost;
    if hud_button(
        Rect::new(x, y, panel.w - 28.0, 34.0),
        &format!("Attract Beetle ({cost} ore) · have {beetles}"),
        session.economy.ore_stock >= cost,
        mouse,
    ) {
        actions.push(UiAction::AttractBeetle);
    }
}

fn draw_tools_panel(
    session: &GameSession,
    data: &GameData,
    panel: Rect,
    mode: &UiMode,
    mouse: Vec2,
    actions: &mut Vec<UiAction>,
) {
    draw_surface_with_title(
        panel,
        Some("Build & Dig"),
        &panel_style(),
        TextStyle::new(17.0, dark::TEXT),
    );

    let x = panel.x + 14.0;
    let mut y = panel.y + 44.0;
    let w = panel.w - 28.0;

    let mut defs: Vec<_> = data.buildings.iter().filter(|(_, d)| d.buildable).collect();
    defs.sort_by(|a, b| a.0.cmp(b.0));
    for (id, def) in defs {
        let active = *mode == UiMode::Build(id.clone());
        let label = format!(
            "{}{} ({} ore)",
            if active { "▶ " } else { "" },
            def.name,
            def.cost_ore
        );
        if hud_button(Rect::new(x, y, w, 30.0), &label, true, mouse) {
            actions.push(UiAction::SetMode(UiMode::Build(id.clone())));
        }
        y += 36.0;
    }

    let dig_active = *mode == UiMode::Dig;
    let dig_label = if dig_active {
        "▶ Dig (toggle marks)"
    } else {
        "Dig (toggle marks)"
    };
    if hud_button(Rect::new(x, y, w, 30.0), dig_label, true, mouse) {
        actions.push(UiAction::SetMode(UiMode::Dig));
    }
    y += 40.0;

    let half = (w - 8.0) / 2.0;
    if hud_button(Rect::new(x, y, half, 30.0), "Save (F5)", true, mouse) {
        actions.push(UiAction::Save);
    }
    if hud_button(
        Rect::new(x + half + 8.0, y, half, 30.0),
        "Load (F9)",
        true,
        mouse,
    ) {
        actions.push(UiAction::Load);
    }

    // Show pending construction so hauling progress is visible.
    if !session.build_sites.is_empty() {
        let pending: u32 = session.build_sites.iter().map(|s| s.remaining()).sum();
        draw_ui_text_ex(
            &format!(
                "{} site(s) awaiting {} ore",
                session.build_sites.len(),
                pending
            ),
            x,
            y + 52.0,
            TextStyle::new(14.0, dark::TEXT_DIM).params(),
        );
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
            session.economy.ore_delivered_total, minutes
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

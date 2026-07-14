//! The standing HUD chrome: the top bar, the calorie balance meter, the
//! job assignment panel, the build/dig tools, and the tutorial card.

use crate::data::GameData;
use crate::simulation::{self, food};
use crate::state::creatures::Job;
use crate::state::GameSession;
use crate::ui::hud::widgets::{hud_button, panel_style};
use crate::ui::{UiAction, UiMode, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, format_mmss};

pub(super) fn draw_top_bar(
    session: &GameSession,
    bar: Rect,
    mouse: Vec2,
    actions: &mut Vec<UiAction>,
) {
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
        &format_mmss(seconds),
        bar.x + 290.0,
        bar.y + 31.0,
        TextStyle::new(18.0, dark::TEXT).params(),
    );

    if session.raid_active {
        draw_ui_text_ex(
            "RAID — gnarls are after the larder!",
            bar.x + 380.0,
            bar.y + 31.0,
            TextStyle::new(18.0, dark::NEGATIVE).params(),
        );
    } else if session.economy.food <= 0.0 {
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
    if hud_button(
        Rect::new(bar.right() - 254.0, bar.y + 8.0, 74.0, 32.0),
        "Save F5",
        true,
        mouse,
    ) {
        actions.push(UiAction::Save);
    }
    if hud_button(
        Rect::new(bar.right() - 176.0, bar.y + 8.0, 74.0, 32.0),
        "Load F9",
        true,
        mouse,
    ) {
        actions.push(UiAction::Load);
    }
}

/// The calorie balance meter — production, consumption, and stockpile,
/// exactly like a power graph.
pub(super) fn draw_food_grid_panel(session: &GameSession, data: &GameData, panel: Rect) {
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
    let mut y = panel.y + 50.0;

    draw_ui_text_ex(
        &format!("Production  +{production:.1}/min"),
        x,
        y,
        TextStyle::new(15.0, dark::POSITIVE).params(),
    );
    y += 21.0;
    draw_ui_text_ex(
        &format!("Upkeep      -{consumption:.1}/min"),
        x,
        y,
        TextStyle::new(15.0, dark::NEGATIVE).params(),
    );
    y += 21.0;
    let net_color = if net >= 0.0 {
        dark::POSITIVE
    } else {
        dark::NEGATIVE
    };
    draw_ui_text_ex(
        &format!("Net         {net:+.1}/min"),
        x,
        y,
        TextStyle::new(15.0, net_color).params(),
    );
    y += 15.0;

    meter(
        Rect::new(x, y, panel.w - 28.0, 18.0),
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
    y += 30.0;

    // Chain throughput + haul pressure: the food grid generalised to a
    // factory dashboard (plan §Phase 9).
    let hauls = crate::ui::legibility::pending_hauls(session);
    draw_ui_text_ex(
        &format!(
            "Ore +{:.0}/m · Ingots +{:.0}/m · Hauls {hauls}",
            session.economy.ore_ema_per_min.max(0.0),
            session.economy.ingot_ema_per_min.max(0.0),
        ),
        x,
        y,
        TextStyle::new(14.0, dark::TEXT_DIM).params(),
    );
    y += 20.0;
    draw_ui_text_ex(
        &format!(
            "Ore banked {} · delivered {}/{}",
            session.economy.ore_stock,
            session.economy.ore_delivered_total,
            data.balance.win_ore_delivered
        ),
        x,
        y,
        TextStyle::new(14.0, dark::TEXT).params(),
    );
    y += 20.0;
    let ingot_color = if session.economy.ingots_forged > 0 {
        dark::TEXT
    } else {
        dark::TEXT_DIM
    };
    let shrine_built = session.buildings_of("worm_shrine").next().is_some();
    let worm_note = if session.worm_awake {
        " · Worm AWAKE".to_owned()
    } else if shrine_built {
        format!(
            " · Worm {:.0}/{:.0}",
            session.worm_fed, data.balance.worm_awaken_at
        )
    } else {
        String::new()
    };
    draw_ui_text_ex(
        &format!(
            "Ingots {}/{} (banked {}) · Captured {} · Raids {}{}",
            session.economy.ingots_forged,
            data.balance.win2_ingots,
            session.economy.ingots_stock,
            session.progress.beetles_captured,
            session.progress.raids_survived,
            worm_note
        ),
        x,
        y,
        TextStyle::new(14.0, ingot_color).params(),
    );
}

pub(super) fn draw_jobs_panel(
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
    let mut y = panel.y + 44.0;

    for job in [Job::Miner, Job::Carrier, Job::Cook, Job::Smith, Job::Guard] {
        let count = session.job_count(job);
        draw_ui_text_ex(
            &format!("{:<8}{count}", job.label()),
            x,
            y + 19.0,
            TextStyle::new(16.0, dark::TEXT).params(),
        );
        if hud_button(Rect::new(x + 130.0, y, 34.0, 26.0), "-", count > 0, mouse) {
            actions.push(UiAction::Unassign(job));
        }
        if hud_button(Rect::new(x + 172.0, y, 34.0, 26.0), "+", idle > 0, mouse) {
            actions.push(UiAction::Assign(job));
        }
        y += 32.0;
    }

    draw_ui_text_ex(
        &format!("Idle    {idle}"),
        x,
        y + 18.0,
        TextStyle::new(16.0, dark::TEXT_DIM).params(),
    );
    y += 28.0;

    let beetles = session
        .creatures
        .iter()
        .filter(|c| c.species == "beetle")
        .count();
    let salamanders = session
        .creatures
        .iter()
        .filter(|c| c.species == "salamander")
        .count();
    let half = (panel.w - 36.0) / 2.0;
    if hud_button(
        Rect::new(x, y, half, 30.0),
        &format!("Beetle {} ({})", beetles, data.balance.beetle_ore_cost),
        session.economy.ore_stock >= data.balance.beetle_ore_cost,
        mouse,
    ) {
        actions.push(UiAction::AttractBeetle);
    }
    let has_den = session.buildings_of("smelter").next().is_some();
    if hud_button(
        Rect::new(x + half + 8.0, y, half, 30.0),
        &format!(
            "Salam. {} ({})",
            salamanders, data.balance.salamander_ore_cost
        ),
        has_den && session.economy.ore_stock >= data.balance.salamander_ore_cost,
        mouse,
    ) {
        actions.push(UiAction::AttractSalamander);
    }
}

pub(super) fn draw_tools_panel(
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
    let mut y = panel.y + 34.0;
    let w = panel.w - 28.0;
    let half = (w - 8.0) / 2.0;

    // Build buttons, two per row: label is the short name + cost. Locked
    // kinds stay visible but disabled (progression is discoverable).
    let mut defs: Vec<_> = data.buildings.iter().filter(|(_, d)| d.buildable).collect();
    defs.sort_by(|a, b| a.0.cmp(b.0));
    for pair in defs.chunks(2) {
        for (i, (id, def)) in pair.iter().enumerate() {
            let active = *mode == UiMode::Build((*id).clone());
            let unlocked = session.building_unlocked(def);
            let short = def.name.split_whitespace().last().unwrap_or(&def.name);
            let label = if unlocked {
                format!(
                    "{}{short} ({})",
                    if active { "▶ " } else { "" },
                    def.cost_ore
                )
            } else {
                format!("{short} 🔒")
            };
            let bx = x + (half + 8.0) * i as f32;
            if hud_button(Rect::new(bx, y, half, 22.0), &label, unlocked, mouse) {
                actions.push(UiAction::SetMode(UiMode::Build((*id).clone())));
            }
        }
        y += 26.0;
    }

    let dig_active = *mode == UiMode::Dig;
    let dig_label = if dig_active { "▶ Dig" } else { "Dig" };
    if hud_button(Rect::new(x, y, half, 22.0), dig_label, true, mouse) {
        actions.push(UiAction::SetMode(UiMode::Dig));
    }
    // Show pending construction so hauling progress is visible.
    if !session.build_sites.is_empty() {
        let pending: u32 = session.build_sites.iter().map(|s| s.remaining()).sum();
        draw_ui_text_ex(
            &format!("{} site(s) · {} ore", session.build_sites.len(), pending),
            x + half + 8.0,
            y + 16.0,
            TextStyle::new(13.0, dark::TEXT_DIM).params(),
        );
    }
}

/// The tutorial card, top-right: current step, progress chip, and a skip
/// button. Returns its rect while visible (for pointer-over-UI checks).
pub(super) fn draw_tutorial_panel(
    session: &GameSession,
    data: &GameData,
    mouse: Vec2,
    actions: &mut Vec<UiAction>,
) -> Option<Rect> {
    let step = crate::tutorial::current_step(session, data)?;
    let (done, total) = crate::tutorial::progress(session, data);

    let panel = Rect::new(LOGICAL_WIDTH - 342.0, 72.0, 330.0, 128.0);
    draw_surface_with_title(
        panel,
        Some(&format!("Tutorial {}/{} — {}", done + 1, total, step.title)),
        &panel_style(),
        TextStyle::new(15.0, dark::TEXT_BRIGHT),
    );

    draw_text_block(
        &step.body,
        panel.x + 14.0,
        panel.y + 42.0,
        panel.w - 28.0,
        56.0,
        14.0,
        4.0,
        dark::TEXT,
    );

    if hud_button(
        Rect::new(panel.right() - 78.0, panel.bottom() - 30.0, 64.0, 22.0),
        "Skip",
        true,
        mouse,
    ) {
        actions.push(UiAction::SkipTutorial);
    }

    Some(panel)
}

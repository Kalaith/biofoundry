//! Screen-space HUD: the calorie balance meter (the game's "power UI"),
//! job assignment panel, build tools, and victory overlay. Pure view —
//! returns intents and whether the pointer is over HUD chrome.

use crate::data::GameData;
use crate::simulation::{self, food};
use crate::state::creatures::{Good, Job, Task};
use crate::state::GameSession;
use crate::ui::{HudFrame, UiAction, UiMode, LOGICAL_HEIGHT, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::grid::TilePos;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::draw_ui_text_ex;

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

    draw_top_bar(session, top_bar, mouse, &mut actions);
    draw_food_grid_panel(session, data, food_panel);
    draw_jobs_panel(session, data, jobs_panel, mouse, &mut actions);
    draw_tools_panel(session, data, tools_panel, mode, mouse, &mut actions);
    let tutorial_panel = draw_tutorial_panel(session, data, mouse, &mut actions);
    let inspect_panel =
        selected.and_then(|pos| draw_inspect_panel(session, data, pos, mouse, &mut actions));

    // A status-icon legend, shown only while some node is stalled — it
    // teaches the in-world badges exactly when they matter.
    if session
        .buildings
        .iter()
        .any(|b| crate::ui::legibility::building_status(session, data, b).is_some())
    {
        draw_status_legend();
    }

    let victory_up = session.won && !session.victory_shown;
    let factory_up = session.factory_complete && !session.factory_shown;
    let worm_up = session.worm_awake && !session.worm_shown;
    if worm_up {
        draw_goal_overlay(
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
        draw_goal_overlay(
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
        draw_goal_overlay(
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
fn draw_tutorial_panel(
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

/// First-pass building inspection (plan §Phase 6): what a clicked building
/// is doing right now. Phase 9 grows this into the full legibility layer.
/// Returns its rect while a building is selected.
fn draw_inspect_panel(
    session: &GameSession,
    data: &GameData,
    pos: TilePos,
    mouse: Vec2,
    actions: &mut Vec<UiAction>,
) -> Option<Rect> {
    let building = session.building_at(pos)?;
    let def = data.buildings.get(&building.kind);
    let name = def.map(|d| d.name.as_str()).unwrap_or(&building.kind);

    // The blacksmith panel carries the production-order queue and craft
    // buttons, so it's taller.
    let height = if building.kind == "blacksmith" {
        150.0 + data.equipment.len() as f32 * 26.0
    } else {
        132.0
    };
    let panel = Rect::new(LOGICAL_WIDTH - 262.0, 210.0, 250.0, height);
    draw_surface_with_title(
        panel,
        Some(name),
        &panel_style(),
        TextStyle::new(16.0, dark::TEXT_BRIGHT),
    );

    let x = panel.x + 14.0;
    let mut y = panel.y + 50.0;
    let line = |text: &str, color: Color, y: &mut f32| {
        draw_ui_text_ex(text, x, *y, TextStyle::new(14.0, color).params());
        *y += 20.0;
    };

    match building.kind.as_str() {
        "mine" => {
            let staffed = session
                .creatures
                .iter()
                .filter(|c| matches!(&c.task, Task::WorkMine(p) if *p == pos))
                .count();
            let slots = def
                .and_then(|d| d.workstation.as_ref())
                .map(|w| w.slots)
                .unwrap_or(0);
            // Effective rate folds in each stationed miner's Iron Pickaxe.
            let rate: f32 = session
                .creatures
                .iter()
                .filter(|c| matches!(&c.task, Task::WorkMine(p) if *p == pos))
                .map(|c| {
                    let mult = c
                        .equipment
                        .as_deref()
                        .and_then(|id| data.equipment_def(id))
                        .filter(|e| e.effect == "mine_speed_mult")
                        .map(|e| e.value)
                        .unwrap_or(1.0);
                    data.balance.mine_ore_per_min * mult
                })
                .sum();
            let (worker_txt, worker_col) = if building.reserve <= 0.0 {
                ("Deposit exhausted".to_owned(), dark::NEGATIVE)
            } else if staffed == 0 {
                ("No miner — stopped".to_owned(), dark::WARNING)
            } else {
                (format!("Miners {staffed}/{slots}"), dark::POSITIVE)
            };
            line(&worker_txt, worker_col, &mut y);
            line(&format!("Ore  +{rate:.0}/min"), dark::TEXT, &mut y);
            line(
                &format!(
                    "Buffer {:.0}/{:.0}",
                    building.stock(Good::Ore),
                    data.balance.mine_buffer_cap
                ),
                dark::TEXT,
                &mut y,
            );
            line(
                &format!("Reserve {:.0}", building.reserve.max(0.0)),
                dark::TEXT_DIM,
                &mut y,
            );
        }
        "farm" => {
            line(
                &format!(
                    "Mushrooms {:.0}/{:.0}",
                    building.stock(Good::Mushroom),
                    crate::simulation::wildlife::farm_cap(session, data)
                ),
                dark::TEXT,
                &mut y,
            );
            line("Carriers haul to the Cook Pot", dark::TEXT_DIM, &mut y);
        }
        "cook_pot" => {
            line(
                &format!("Mushrooms {:.0}", building.stock(Good::Mushroom)),
                dark::TEXT,
                &mut y,
            );
            line("Cooks turn mushrooms → stew", dark::TEXT_DIM, &mut y);
        }
        "blacksmith" => {
            let staffed = session.creatures.iter().any(|c| {
                matches!(&c.task, Task::Smithing { shop, .. } if *shop == pos)
                    || matches!(&c.task, Task::Crafting { shop, .. } if *shop == pos)
            });
            line(
                if staffed {
                    "Smith at work"
                } else {
                    "No smith — idle"
                },
                if staffed {
                    dark::POSITIVE
                } else {
                    dark::WARNING
                },
                &mut y,
            );
            line(
                &format!(
                    "Ore {:.0}  Ingots {:.0}  Queue {}",
                    building.stock(Good::Ore),
                    building.stock(Good::Ingot),
                    building.orders.len()
                ),
                dark::TEXT,
                &mut y,
            );
            // Production orders: one craft button per equipment item. A
            // queued count and how many are already banked ride in the label.
            y += 2.0;
            let bw = panel.w - 28.0;
            for eq in &data.equipment {
                let banked = session.economy.gear_stock.get(&eq.id).copied().unwrap_or(0);
                let queued = building.orders.iter().filter(|o| **o == eq.id).count();
                let mut label = format!("{} ({})", eq.name, eq.cost_ingots);
                if queued > 0 {
                    label.push_str(&format!("  ·{queued} queued"));
                } else if banked > 0 {
                    label.push_str(&format!("  ·{banked} ready"));
                }
                if hud_button(Rect::new(x, y, bw, 22.0), &label, true, mouse) {
                    actions.push(UiAction::QueueOrder(pos, eq.id.clone()));
                }
                y += 26.0;
            }
        }
        "kiln" => {
            line(
                &format!(
                    "Wood {:.0}  Charcoal {:.0}",
                    building.stock(Good::Wood),
                    building.stock(Good::Charcoal)
                ),
                dark::TEXT,
                &mut y,
            );
        }
        "smelter" => {
            line(
                &format!(
                    "Ore {:.0}  Charcoal {:.0}",
                    building.stock(Good::Ore),
                    building.stock(Good::Charcoal)
                ),
                dark::TEXT,
                &mut y,
            );
        }
        _ => {
            line("Click empty ground to deselect", dark::TEXT_DIM, &mut y);
        }
    }

    Some(panel)
}

fn draw_goal_overlay(
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
fn draw_status_legend() {
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

//! The building inspection card: what the clicked node is doing right now,
//! plus its per-kind verbs (blacksmith production orders, pit breeding).

use crate::data::GameData;
use crate::state::creatures::{Good, Task};
use crate::state::GameSession;
use crate::ui::hud::widgets::{hud_button, panel_style};
use crate::ui::{UiAction, LOGICAL_WIDTH};
use macroquad::prelude::*;
use macroquad_toolkit::grid::TilePos;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::draw_ui_text_ex;

/// First-pass building inspection (plan §Phase 6): what a clicked building
/// is doing right now. Phase 9 grows this into the full legibility layer.
/// Returns its rect while a building is selected.
pub(super) fn draw_inspect_panel(
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
    // buttons, and the breeding pit its breed buttons — both taller.
    let height = match building.kind.as_str() {
        "blacksmith" => 150.0 + data.equipment.len() as f32 * 26.0,
        "breeding_pit" => 172.0,
        _ => 132.0,
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
                    let pickaxe = c
                        .equipment
                        .as_deref()
                        .and_then(|id| data.equipment_def(id))
                        .filter(|e| e.effect == "mine_speed_mult")
                        .map(|e| e.value)
                        .unwrap_or(1.0);
                    // Species strength and any Overseer aura fold in too.
                    data.balance.mine_ore_per_min
                        * pickaxe
                        * crate::ui::legibility::work_multiplier(c, session, data)
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
        "breeding_pit" => {
            line(
                "Hatches beetles from studied stock.",
                dark::TEXT_DIM,
                &mut y,
            );
            // Evolution line: breed heavyweight workers once forged ingots
            // unlock them. Buttons stay visible, disabled until then.
            let bw = panel.w - 28.0;
            y += 2.0;
            let has_overseer = session.creatures.iter().any(|c| c.species == "overseer");
            for (id, unlock, cost, blocked) in [
                (
                    "hobgoblin",
                    "hobgoblin",
                    data.balance.hobgoblin_ingot_cost,
                    false,
                ),
                (
                    "overseer",
                    "overseer",
                    data.balance.overseer_ingot_cost,
                    has_overseer,
                ),
            ] {
                let unlocked = session.unlocked.contains(unlock);
                let name = data.species.get(id).map(|s| s.name.as_str()).unwrap_or(id);
                let label = if !unlocked {
                    format!("{name} 🔒")
                } else if blocked {
                    format!("{name} — posted")
                } else {
                    format!("Breed {name} ({cost} ingots)")
                };
                let enabled = unlocked && !blocked && session.economy.ingots_stock >= cost;
                if hud_button(Rect::new(x, y, bw, 24.0), &label, enabled, mouse) {
                    actions.push(UiAction::Breed(id.to_owned()));
                }
                y += 28.0;
            }
        }
        _ => {
            line("Click empty ground to deselect", dark::TEXT_DIM, &mut y);
        }
    }

    Some(panel)
}

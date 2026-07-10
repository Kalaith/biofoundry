//! UI is a pure view layer: it reads state, draws, and returns `UiAction`
//! intents. It never mutates game state — `Game::apply_action` dispatches.

pub mod hud;
pub mod menu;
pub mod warren;

use crate::state::creatures::Job;
use macroquad_toolkit::grid::TilePos;

pub const LOGICAL_WIDTH: f32 = 1280.0;
pub const LOGICAL_HEIGHT: f32 = 720.0;

/// What a world click means right now.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UiMode {
    #[default]
    Inspect,
    /// Placing a ghost of this building kind (id into `buildings.json`).
    Build(String),
    /// Toggling dig designations on rock.
    Dig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiAction {
    StartWarren,
    BackToMenu,
    /// Move one idle goblin into this job.
    Assign(Job),
    /// Move one goblin out of this job into the idle pool.
    Unassign(Job),
    AttractBeetle,
    AttractSalamander,
    DismissVictory,
    DismissFactory,
    /// Toggle a tool mode (clicking the active mode returns to Inspect).
    SetMode(UiMode),
    /// The player clicked this world tile with the active tool.
    WorldClick(TilePos),
    Save,
    Load,
}

/// One frame of HUD output.
pub struct HudFrame {
    pub actions: Vec<UiAction>,
    /// True when the pointer is over HUD chrome — world clicks should be
    /// ignored while true.
    pub pointer_over_ui: bool,
}

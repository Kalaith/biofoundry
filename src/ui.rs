//! UI is a pure view layer: it reads state, draws, and returns `UiAction`
//! intents. It never mutates game state — `Game::apply_action` dispatches.

pub mod hud;
pub mod menu;
pub mod warren;

use crate::state::creatures::Job;

pub const LOGICAL_WIDTH: f32 = 1280.0;
pub const LOGICAL_HEIGHT: f32 = 720.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiAction {
    StartWarren,
    BackToMenu,
    /// Move one idle goblin into this job.
    Assign(Job),
    /// Move one goblin out of this job into the idle pool.
    Unassign(Job),
    AttractBeetle,
    DismissVictory,
}

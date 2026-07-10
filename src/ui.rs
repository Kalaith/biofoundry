//! UI is a pure view layer: it reads state, draws, and returns `UiAction`
//! intents. It never mutates game state — `Game::apply_action` dispatches.

pub mod menu;
pub mod warren;

pub const LOGICAL_WIDTH: f32 = 1280.0;
pub const LOGICAL_HEIGHT: f32 = 720.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiAction {
    StartWarren,
    BackToMenu,
}

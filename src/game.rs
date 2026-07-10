//! Top-level game: owns the state machine, camera, tool mode, and
//! fixed-timestep accumulator, and dispatches `UiAction` intents.

use crate::data::GameData;
use crate::simulation::{self, MAX_TICKS_PER_FRAME, SIM_DT};
use crate::state::creatures::Job;
use crate::state::{GameSession, GameState, StateTransition};
use crate::ui::{self, UiAction, UiMode};
use macroquad::prelude::*;
use macroquad_toolkit::camera::{Camera2D, Camera2DConfig, CameraBounds};
use macroquad_toolkit::events::EventBus;
use macroquad_toolkit::grid::TilePos;
use macroquad_toolkit::notifications::{
    NotificationAnchor, NotificationManager, NotificationRenderConfig,
};
use macroquad_toolkit::persistence::{load_from_slot_with_migration, save_to_slot_with_version};
use macroquad_toolkit::prelude::{begin_virtual_ui_frame, dark, end_virtual_ui_frame, InputState};

pub struct Game {
    data: GameData,
    state: GameState,
    camera: Camera2D,
    mode: UiMode,
    events: EventBus<UiAction>,
    notifications: NotificationManager,
    /// Real time not yet consumed by fixed-step sim ticks.
    accumulator: f32,
    /// Edge detector for the famine warning toast.
    famine_announced: bool,
}

impl Game {
    pub async fn new() -> Self {
        let data = GameData::load().unwrap_or_else(|err| {
            panic!("Biofoundry embedded data failed to load: {}", err);
        });

        let camera = Camera2D::with_config(vec2(0.0, 0.0), 1.0, camera_config(&data, 1.0));

        Self {
            data,
            state: GameState::Menu,
            camera,
            mode: UiMode::Inspect,
            events: EventBus::new(),
            notifications: NotificationManager::new(),
            accumulator: 0.0,
            famine_announced: false,
        }
    }

    /// Seed a named scene for the headless screenshot harness.
    pub fn begin_capture_scene(&mut self, scene: &str) {
        match scene {
            "menu" => self.transition(StateTransition::BackToMenu),
            "factory" => {
                self.transition(StateTransition::StartWarren);
                if let GameState::Warren(session) = &mut self.state {
                    // Stage a mid-build factory: banked ore, ghosts, digs.
                    session.economy.ore_stock = 24;
                    session.economy.food = 60.0;
                    let spawn = session.spawn_tile();
                    let mut spots: Vec<TilePos> = session
                        .world
                        .tiles
                        .iter_with_pos()
                        .filter(|(pos, _)| session.can_place_building(*pos))
                        .map(|(pos, _)| pos)
                        .collect();
                    spots.sort_by_key(|p| (p.manhattan_distance(&spawn), p.x, p.y));
                    for (kind, spot) in ["farm", "cook_pot"].iter().zip(spots.iter().skip(2)) {
                        simulation::try_place_build_site(session, &self.data, kind, *spot);
                    }
                    for mark in session
                        .world
                        .tiles
                        .iter_with_pos()
                        .filter(|(_, t)| **t == crate::state::world::Tile::Rock)
                        .map(|(pos, _)| pos)
                        .filter(|p| p.manhattan_distance(&spawn) <= 6)
                        .take(4)
                        .collect::<Vec<_>>()
                    {
                        session.toggle_dig_mark(mark);
                    }
                    for _ in 0..900 {
                        simulation::tick(session, &self.data);
                    }
                }
            }
            "famine" => {
                self.transition(StateTransition::StartWarren);
                if let GameState::Warren(session) = &mut self.state {
                    for _ in 0..600 {
                        simulation::tick(session, &self.data);
                    }
                    session.economy.food = 0.0;
                    for creature in &mut session.creatures {
                        creature.satiation = 0.3;
                    }
                    for _ in 0..100 {
                        simulation::tick(session, &self.data);
                    }
                }
            }
            // "warren" and the harness default "gameplay" boot straight
            // into a fresh session on the config seed.
            _ => self.transition(StateTransition::StartWarren),
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.notifications.update(dt);
        let input = InputState::capture();

        if let GameState::Warren(session) = &mut self.state {
            self.accumulator += dt;
            let mut ticks = 0;
            while self.accumulator >= SIM_DT && ticks < MAX_TICKS_PER_FRAME {
                let report = simulation::tick(session, &self.data);
                for deserter in &report.deserters {
                    self.notifications.danger(format!(
                        "A starving {} deserted the warren!",
                        deserter.job.label().to_lowercase()
                    ));
                }
                if report.won_this_tick {
                    self.notifications.success("The warren thrives — victory!");
                }
                if report.factory_this_tick {
                    self.notifications
                        .success("The Biofoundry roars — factory complete!");
                }
                self.accumulator -= SIM_DT;
                ticks += 1;
            }
            // Drop backlog beyond the cap instead of spiraling.
            if self.accumulator >= SIM_DT {
                self.accumulator = 0.0;
            }

            if session.economy.food <= 0.0 && !self.famine_announced {
                self.famine_announced = true;
                self.notifications
                    .warning("Famine! The stockpile is empty — workers are slowing.");
            } else if session.economy.food > 5.0 {
                self.famine_announced = false;
            }

            self.camera.update(dt, false);
            // F-keys avoid colliding with WASD camera pan.
            if is_key_pressed(KeyCode::F5) {
                self.events.push(UiAction::Save);
            }
            if is_key_pressed(KeyCode::F9) {
                self.events.push(UiAction::Load);
            }
            if input.escape_pressed {
                // Escape backs out of a tool first, then to the menu.
                if self.mode != UiMode::Inspect {
                    self.mode = UiMode::Inspect;
                } else {
                    self.events.push(UiAction::BackToMenu);
                }
            }
        }

        let actions: Vec<UiAction> = self.events.drain().collect();
        for action in actions {
            self.apply_action(action);
        }
    }

    pub fn draw(&mut self) {
        clear_background(dark::BACKGROUND);

        let actions = match &self.state {
            GameState::Menu => {
                let virtual_ui = begin_virtual_ui_frame(ui::LOGICAL_WIDTH, ui::LOGICAL_HEIGHT);
                let actions = ui::menu::draw(&self.data, &virtual_ui);
                end_virtual_ui_frame();
                actions
            }
            GameState::Warren(session) => {
                let hover = self.hover_tile(session);

                self.camera.begin();
                ui::warren::draw_world(session, self.data.config.tile_size, &self.mode, hover);
                set_default_camera();

                let virtual_ui = begin_virtual_ui_frame(ui::LOGICAL_WIDTH, ui::LOGICAL_HEIGHT);
                let frame = ui::hud::draw(session, &self.data, &virtual_ui, &self.mode);
                end_virtual_ui_frame();

                let mut actions = frame.actions;
                if !frame.pointer_over_ui
                    && self.mode != UiMode::Inspect
                    && is_mouse_button_released(MouseButton::Left)
                {
                    if let Some(tile) = hover {
                        actions.push(UiAction::WorldClick(tile));
                    }
                }
                actions
            }
        };

        for action in actions {
            self.events.push(action);
        }

        self.notifications
            .draw_with_config(&NotificationRenderConfig {
                anchor: NotificationAnchor::BottomRight,
                ..Default::default()
            });
    }

    /// World tile under the mouse cursor, if inside the map.
    fn hover_tile(&self, session: &GameSession) -> Option<TilePos> {
        let world = self.camera.screen_to_world(mouse_position().into());
        let ts = self.data.config.tile_size;
        let tile = TilePos::new((world.x / ts).floor() as i32, (world.y / ts).floor() as i32);
        session.world.tiles.is_valid(tile).then_some(tile)
    }

    fn apply_action(&mut self, action: UiAction) {
        match action {
            UiAction::StartWarren => self.transition(StateTransition::StartWarren),
            UiAction::BackToMenu => self.transition(StateTransition::BackToMenu),
            UiAction::Assign(job) => self.reassign(Job::Idle, job),
            UiAction::Unassign(job) => self.reassign(job, Job::Idle),
            UiAction::AttractBeetle => {
                if let GameState::Warren(session) = &mut self.state {
                    if simulation::try_attract_beetle(session, &self.data) {
                        self.notifications
                            .success("A beetle hauler joins the warren.");
                    } else {
                        self.notifications.warning("Not enough ore banked.");
                    }
                }
            }
            UiAction::AttractSalamander => {
                if let GameState::Warren(session) = &mut self.state {
                    if simulation::try_attract_salamander(session, &self.data) {
                        self.notifications
                            .success("A salamander curls into the smelter den.");
                    } else {
                        self.notifications
                            .warning("Needs a Smelter Den and enough banked ore.");
                    }
                }
            }
            UiAction::DismissVictory => {
                if let GameState::Warren(session) = &mut self.state {
                    session.victory_shown = true;
                }
            }
            UiAction::DismissFactory => {
                if let GameState::Warren(session) = &mut self.state {
                    session.factory_shown = true;
                }
            }
            UiAction::SetMode(mode) => {
                self.mode = if self.mode == mode {
                    UiMode::Inspect
                } else {
                    mode
                };
            }
            UiAction::WorldClick(tile) => self.world_click(tile),
            UiAction::Save => self.save_game(),
            UiAction::Load => self.load_game(),
        }
    }

    fn world_click(&mut self, tile: TilePos) {
        let mode = self.mode.clone();
        let GameState::Warren(session) = &mut self.state else {
            return;
        };
        match mode {
            UiMode::Build(kind) => {
                if simulation::try_place_build_site(session, &self.data, &kind, tile) {
                    let cost = self
                        .data
                        .buildings
                        .get(&kind)
                        .map(|d| d.cost_ore)
                        .unwrap_or(0);
                    self.notifications
                        .info(format!("Site placed — carriers will deliver {cost} ore."));
                } else {
                    self.notifications.warning("Can't build there.");
                }
            }
            UiMode::Dig => {
                session.toggle_dig_mark(tile);
            }
            UiMode::Inspect => {}
        }
    }

    fn save_game(&mut self) {
        let GameState::Warren(session) = &self.state else {
            return;
        };
        let config = &self.data.config;
        match save_to_slot_with_version(
            &config.game_name,
            &config.save_slot,
            session.as_ref(),
            &config.version,
        ) {
            Ok(()) => self.notifications.success("Warren saved."),
            Err(err) => self.notifications.danger(format!("Save failed: {err}")),
        }
    }

    fn load_game(&mut self) {
        let config = &self.data.config;
        let loaded: Result<GameSession, String> = load_from_slot_with_migration(
            &config.game_name,
            &config.save_slot,
            &config.version,
            |version, value| {
                let payload = value.get("data").cloned().unwrap_or(value);
                serde_json::from_value(payload)
                    .map_err(|err| format!("Unsupported save {version:?}: {err}"))
            },
        );

        match loaded {
            Ok(session) => {
                self.reset_camera_for(&session);
                self.accumulator = 0.0;
                self.mode = UiMode::Inspect;
                self.state = GameState::Warren(Box::new(session));
                self.notifications.success("Warren loaded.");
            }
            Err(err) => self.notifications.warning(format!("Load failed: {err}")),
        }
    }

    fn reassign(&mut self, from: Job, to: Job) {
        let GameState::Warren(session) = &mut self.state else {
            return;
        };
        let species = &self.data.species;
        session.reassign(from, to, |s| {
            species.get(s).map(|d| d.reassignable).unwrap_or(false)
        });
    }

    fn transition(&mut self, transition: StateTransition) {
        match transition {
            StateTransition::StartWarren => {
                let session = GameSession::new(&self.data, self.data.config.world_seed);
                self.reset_camera_for(&session);
                self.accumulator = 0.0;
                self.famine_announced = false;
                self.mode = UiMode::Inspect;
                self.state = GameState::Warren(Box::new(session));
            }
            StateTransition::BackToMenu => {
                self.mode = UiMode::Inspect;
                self.state = GameState::Menu;
            }
        }
    }

    fn reset_camera_for(&mut self, session: &GameSession) {
        let tile = self.data.config.tile_size;
        let (sx, sy) = session.world.spawn.to_f32();
        let center = vec2((sx + 0.5) * tile, (sy + 0.5) * tile);
        self.camera = Camera2D::with_config(center, 1.0, camera_config(&self.data, tile));
    }
}

fn camera_config(data: &GameData, tile_size: f32) -> Camera2DConfig {
    let world_w = data.config.world_width as f32 * tile_size;
    let world_h = data.config.world_height as f32 * tile_size;
    Camera2DConfig {
        drag_button: Some(MouseButton::Right),
        min_zoom: 0.5,
        max_zoom: 3.0,
        bounds: Some(CameraBounds::new(vec2(0.0, 0.0), vec2(world_w, world_h))),
        ..Default::default()
    }
}

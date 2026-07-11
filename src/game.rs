//! Top-level game: owns the state machine, camera, tool mode, and
//! fixed-timestep accumulator, and dispatches `UiAction` intents.

use crate::audio::{Audio, Sfx};
use crate::data::GameData;
use crate::simulation::{self, MAX_TICKS_PER_FRAME, SIM_DT};
use crate::state::creatures::Job;
use crate::state::{GameSession, GameState, StateTransition};
use crate::tutorial::{self, TutorialInputs};
use crate::ui::{self, UiAction, UiMode};
use macroquad::prelude::*;
use macroquad_toolkit::camera::{Camera2D, Camera2DConfig, CameraBounds};
use macroquad_toolkit::events::EventBus;
use macroquad_toolkit::grid::TilePos;
use macroquad_toolkit::notifications::{
    NotificationAnchor, NotificationManager, NotificationRenderConfig,
};
use macroquad_toolkit::persistence::{
    load_from_slot_with_migration, save_to_slot_with_version, slot_exists,
};
use macroquad_toolkit::prelude::{begin_virtual_ui_frame, dark, end_virtual_ui_frame, InputState};

pub struct Game {
    data: GameData,
    state: GameState,
    camera: Camera2D,
    mode: UiMode,
    events: EventBus<UiAction>,
    notifications: NotificationManager,
    audio: Audio,
    /// Real time not yet consumed by fixed-step sim ticks.
    accumulator: f32,
    /// Edge detector for the famine warning toast.
    famine_announced: bool,
    /// Last frame's camera pose, for the tutorial's "look around" step.
    last_camera: (Vec2, f32),
    /// The title menu's settings panel is showing.
    settings_open: bool,
    /// A save slot exists, so the menu can offer Continue.
    save_exists: bool,
    /// Where the right button went down, to tell a click from a camera drag.
    right_press: Vec2,
}

impl Game {
    pub async fn new() -> Self {
        let data = GameData::load().unwrap_or_else(|err| {
            panic!("Biofoundry embedded data failed to load: {}", err);
        });

        let camera = Camera2D::with_config(vec2(0.0, 0.0), 1.0, camera_config(&data, 1.0));
        let mut audio = Audio::load().await;
        audio.load_settings(&data.config.game_name);
        let save_exists = slot_exists(&data.config.game_name, &data.config.save_slot);

        Self {
            data,
            state: GameState::Menu,
            camera,
            mode: UiMode::Inspect,
            events: EventBus::new(),
            notifications: NotificationManager::new(),
            audio,
            accumulator: 0.0,
            famine_announced: false,
            last_camera: (vec2(0.0, 0.0), 1.0),
            settings_open: false,
            save_exists,
            right_press: vec2(0.0, 0.0),
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
                    session.tutorial_dismissed = true;
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
                    session.tutorial_dismissed = true;
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
            "raid" => {
                self.transition(StateTransition::StartWarren);
                if let GameState::Warren(session) = &mut self.state {
                    // Stage an active raid with guards responding.
                    session.tutorial_dismissed = true;
                    session.economy.food = 60.0;
                    let species = &self.data.species;
                    for _ in 0..2 {
                        session.reassign(Job::Miner, Job::Guard, |s| {
                            species.get(s).map(|d| d.reassignable).unwrap_or(false)
                        });
                    }
                    for _ in 0..300 {
                        simulation::tick(session, &self.data);
                    }
                    session.raid_in = 0.0;
                    for _ in 0..80 {
                        simulation::tick(session, &self.data);
                    }
                }
            }
            "breeding" => {
                self.transition(StateTransition::StartWarren);
                if let GameState::Warren(session) = &mut self.state {
                    // Stage the capture→study→adapt chain mid-flow.
                    session.tutorial_dismissed = true;
                    let spawn = session.spawn_tile();
                    let mut spots: Vec<TilePos> = session
                        .world
                        .tiles
                        .iter_with_pos()
                        .filter(|(pos, _)| session.can_place_building(*pos))
                        .map(|(pos, _)| pos)
                        .collect();
                    spots.sort_by_key(|p| (p.manhattan_distance(&spawn), p.x, p.y));
                    for (kind, spot) in ["trap", "study_pen", "breeding_pit"]
                        .iter()
                        .zip(spots.iter().skip(1))
                    {
                        session
                            .buildings
                            .push(crate::state::structures::Building::new(kind, *spot));
                    }
                    session.progress.beetles_captured = 2;
                    session.progress.specimens = 2;
                    session.wild_spawn_in = 0.0;
                    for _ in 0..200 {
                        simulation::tick(session, &self.data);
                    }
                }
            }
            "worm" => {
                self.transition(StateTransition::StartWarren);
                if let GameState::Warren(session) = &mut self.state {
                    // Stage the awakened monument.
                    session.tutorial_dismissed = true;
                    let spawn = session.spawn_tile();
                    let spot = session
                        .world
                        .tiles
                        .iter_with_pos()
                        .filter(|(pos, _)| {
                            session.can_place_building(*pos) && pos.manhattan_distance(&spawn) >= 3
                        })
                        .map(|(pos, _)| pos)
                        .min_by_key(|p| (p.manhattan_distance(&spawn), p.x, p.y));
                    if let Some(spot) = spot {
                        session
                            .buildings
                            .push(crate::state::structures::Building::new("worm_shrine", spot));
                    }
                    session.economy.metal = self.data.balance.win2_metal;
                    session.won = true;
                    session.victory_shown = true;
                    session.factory_complete = true;
                    session.factory_shown = true;
                    session.worm_fed = self.data.balance.worm_awaken_at;
                    session.worm_awake = true;
                    session.worm_shown = true;
                    for _ in 0..300 {
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
                    self.audio.play(Sfx::Deny);
                }
                if report.won_this_tick {
                    self.notifications.success("The warren thrives — victory!");
                    self.audio.play(Sfx::Complete);
                }
                if report.factory_this_tick {
                    self.notifications
                        .success("The Biofoundry roars — factory complete!");
                    self.audio.play(Sfx::Complete);
                }
                if report.worm_this_tick {
                    self.notifications
                        .success("The ground heaves — the Colossal Worm awakens!");
                    self.audio.play(Sfx::Worm);
                }
                if report.wild.raid_started {
                    self.notifications
                        .danger("Raid! Gnarls are coming for the larder.");
                    self.audio.play(Sfx::Alarm);
                }
                if report.wild.raid_survived {
                    self.notifications.success("The raid is over — we held.");
                    self.audio.play(Sfx::Complete);
                }
                for _ in 0..report.wild.captured {
                    self.notifications
                        .success("A wild beetle was snared — specimen housed.");
                    self.audio.play(Sfx::Capture);
                }
                for _ in 0..report.wild.guards_killed {
                    self.notifications
                        .danger("A guard fell defending the warren.");
                    self.audio.play(Sfx::Deny);
                }
                for name in &report.wild.unlocked {
                    self.notifications.success(format!("Unlocked: {name}"));
                    self.audio.play(Sfx::Complete);
                }
                if report.wild.bred_beetle {
                    self.notifications
                        .info("The breeding pit hatched a new beetle hauler.");
                    self.audio.play(Sfx::Capture);
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
                self.audio.play(Sfx::Alarm);
            } else if session.economy.food > 5.0 {
                self.famine_announced = false;
            }

            self.camera.update(dt, false);

            // Tutorial: advance any steps the player just satisfied. The
            // "look around" step keys off actual camera motion.
            let camera_moved = (self.camera.target - self.last_camera.0).length() > 4.0
                || (self.camera.zoom - self.last_camera.1).abs() > 0.01;
            if tutorial::advance(session, &self.data, TutorialInputs { camera_moved }) {
                self.audio.play(Sfx::Select);
            }
            self.last_camera = (self.camera.target, self.camera.zoom);

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
            // A right-click (not a camera drag) also cancels the tool.
            if is_mouse_button_pressed(MouseButton::Right) {
                self.right_press = mouse_position().into();
            }
            if is_mouse_button_released(MouseButton::Right)
                && self.mode != UiMode::Inspect
                && Vec2::from(mouse_position()).distance(self.right_press) < 8.0
            {
                self.mode = UiMode::Inspect;
            }
        } else if input.escape_pressed && self.settings_open {
            self.settings_open = false;
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
                let actions = ui::menu::draw(
                    &self.data,
                    &virtual_ui,
                    self.save_exists,
                    self.settings_open,
                    self.audio.volume(),
                );
                end_virtual_ui_frame();
                actions
            }
            GameState::Warren(session) => {
                let hover = self.hover_tile(session);

                self.camera.begin();
                ui::warren::draw_world(
                    session,
                    &self.data,
                    self.data.config.tile_size,
                    &self.mode,
                    hover,
                );
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
                        self.audio.play(Sfx::Capture);
                    } else {
                        self.notifications.warning("Not enough ore banked.");
                        self.audio.play(Sfx::Deny);
                    }
                }
            }
            UiAction::AttractSalamander => {
                if let GameState::Warren(session) = &mut self.state {
                    if simulation::try_attract_salamander(session, &self.data) {
                        self.notifications
                            .success("A salamander curls into the smelter den.");
                        self.audio.play(Sfx::Capture);
                    } else {
                        self.notifications
                            .warning("Needs a Smelter Den and enough banked ore.");
                        self.audio.play(Sfx::Deny);
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
            UiAction::DismissWorm => {
                if let GameState::Warren(session) = &mut self.state {
                    session.worm_shown = true;
                }
            }
            UiAction::SkipTutorial => {
                if let GameState::Warren(session) = &mut self.state {
                    session.tutorial_dismissed = true;
                    self.audio.play(Sfx::Select);
                }
            }
            UiAction::SetMode(mode) => {
                self.mode = if self.mode == mode {
                    UiMode::Inspect
                } else {
                    mode
                };
                self.audio.play(Sfx::Select);
            }
            UiAction::WorldClick(tile) => self.world_click(tile),
            UiAction::Save => self.save_game(),
            UiAction::Load => self.load_game(),
            UiAction::ToggleSettings => {
                self.settings_open = !self.settings_open;
                self.audio.play(Sfx::Select);
            }
            UiAction::AdjustVolume(steps) => {
                let volume = (self.audio.volume() * 10.0 + steps as f32).round() / 10.0;
                self.audio.set_volume(volume);
                self.audio.save_settings(&self.data.config.game_name);
                // Chirp at the new level so the change is audible.
                self.audio.play(Sfx::Select);
            }
            UiAction::ExitGame => macroquad::miniquad::window::quit(),
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
                    self.audio.play(Sfx::Build);
                } else {
                    self.notifications.warning("Can't build there.");
                    self.audio.play(Sfx::Deny);
                }
            }
            UiMode::Dig => {
                if session.toggle_dig_mark(tile) {
                    self.audio.play(Sfx::Select);
                }
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
            Ok(()) => {
                self.save_exists = true;
                self.notifications.success("Warren saved.");
            }
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
        if session.reassign(from, to, |s| {
            species.get(s).map(|d| d.reassignable).unwrap_or(false)
        }) {
            self.audio.play(Sfx::Select);
        }
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
        // Don't let the reset itself count as "the player looked around".
        self.last_camera = (self.camera.target, self.camera.zoom);
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

//! Top-level game: owns the state machine, camera, and fixed-timestep
//! accumulator, and dispatches `UiAction` intents from the view layer.

use crate::data::GameData;
use crate::simulation::{self, MAX_TICKS_PER_FRAME, SIM_DT};
use crate::state::creatures::Job;
use crate::state::{GameSession, GameState, StateTransition};
use crate::ui::{self, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::camera::{Camera2D, Camera2DConfig, CameraBounds};
use macroquad_toolkit::events::EventBus;
use macroquad_toolkit::notifications::{
    NotificationAnchor, NotificationManager, NotificationRenderConfig,
};
use macroquad_toolkit::prelude::{begin_virtual_ui_frame, dark, end_virtual_ui_frame, InputState};

pub struct Game {
    data: GameData,
    state: GameState,
    camera: Camera2D,
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
            // "warren" (and the harness default "gameplay") boot straight
            // into a fresh session on the config seed, so captures are
            // deterministic.
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
            if input.escape_pressed {
                self.events.push(UiAction::BackToMenu);
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
                self.camera.begin();
                ui::warren::draw_world(session, self.data.config.tile_size);
                set_default_camera();

                let virtual_ui = begin_virtual_ui_frame(ui::LOGICAL_WIDTH, ui::LOGICAL_HEIGHT);
                let actions = ui::hud::draw(session, &self.data, &virtual_ui);
                end_virtual_ui_frame();
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
                        self.notifications.warning("Not enough ore delivered.");
                    }
                }
            }
            UiAction::DismissVictory => {
                if let GameState::Warren(session) = &mut self.state {
                    session.victory_shown = true;
                }
            }
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
                self.state = GameState::Warren(Box::new(session));
            }
            StateTransition::BackToMenu => {
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

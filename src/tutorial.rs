//! The tutorial: a sequence of data-driven steps (`tutorial.json`) shown
//! in the HUD. Each step completes when the player actually does the
//! thing — pan the camera, reassign a worker, survive the famine, place a
//! building, win. Pure guidance: it reads state and never touches the sim.

use crate::data::{GameData, TutorialDone, TutorialStepDef};
use crate::simulation;
use crate::state::GameSession;

/// Frame-side signals the session can't see (camera input lives in `Game`).
#[derive(Debug, Default, Clone, Copy)]
pub struct TutorialInputs {
    pub camera_moved: bool,
}

/// The step to display, if the tutorial is active.
pub fn current_step<'a>(session: &GameSession, data: &'a GameData) -> Option<&'a TutorialStepDef> {
    if session.tutorial_dismissed {
        return None;
    }
    data.tutorial.get(session.tutorial_step)
}

/// Steps completed so far out of the total (for the "2/6" chip).
pub fn progress(session: &GameSession, data: &GameData) -> (usize, usize) {
    (session.tutorial_step, data.tutorial.len())
}

/// Advance past every step whose condition is now met. Returns true when
/// at least one step completed this call (so the UI can chirp once).
pub fn advance(session: &mut GameSession, data: &GameData, inputs: TutorialInputs) -> bool {
    let mut advanced = false;
    while let Some(step) = current_step(session, data) {
        if !step_done(&step.done, session, inputs) {
            break;
        }
        session.tutorial_step += 1;
        advanced = true;
    }
    advanced
}

fn step_done(done: &TutorialDone, session: &GameSession, inputs: TutorialInputs) -> bool {
    let sim_time = simulation::sim_seconds(session);
    match done {
        TutorialDone::CameraMoved => inputs.camera_moved,
        TutorialDone::SimTimeAtLeast { value } => sim_time >= *value,
        TutorialDone::AnyReassign => session.tutorial_reassigned,
        // "Famine weathered": past the first-crisis window with the larder
        // healthy again — whether the player dodged it or dug out of it.
        TutorialDone::FamineRecovered { value } => {
            sim_time >= 330.0 && session.economy.food >= *value && !session.famine_active
        }
        TutorialDone::SitePlaced => session.tutorial_built,
        TutorialDone::Won => session.won,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::creatures::Job;

    fn boot() -> (GameData, GameSession) {
        let data = GameData::load().unwrap();
        let session = GameSession::new(&data, 42);
        (data, session)
    }

    #[test]
    fn tutorial_data_loads_in_teaching_order() {
        let (data, _) = boot();
        assert!(data.tutorial.len() >= 4, "expected a real tutorial");
        assert_eq!(data.tutorial.first().unwrap().id, "welcome");
        assert!(matches!(
            data.tutorial.last().unwrap().done,
            TutorialDone::Won
        ));
    }

    #[test]
    fn steps_complete_from_player_actions() {
        let (data, mut session) = boot();
        let none = TutorialInputs::default();

        // Step 1 waits for camera input.
        assert_eq!(current_step(&session, &data).unwrap().id, "welcome");
        assert!(!advance(&mut session, &data, none));
        assert!(advance(
            &mut session,
            &data,
            TutorialInputs { camera_moved: true }
        ));
        assert_eq!(current_step(&session, &data).unwrap().id, "food_grid");

        // Step 2 is time-gated; step 3 needs a reassign.
        session.tick = (70.0 / simulation::SIM_DT) as u64;
        assert!(advance(&mut session, &data, none));
        assert_eq!(current_step(&session, &data).unwrap().id, "jobs");
        let moved = session.reassign(Job::Miner, Job::Carrier, |_| true);
        assert!(moved);
        assert!(advance(&mut session, &data, none));
        assert_eq!(current_step(&session, &data).unwrap().id, "famine");

        // Famine step: needs the crisis window past and food healthy.
        session.economy.food = 50.0;
        assert!(!advance(&mut session, &data, none), "too early to count");
        session.tick = (400.0 / simulation::SIM_DT) as u64;
        assert!(advance(&mut session, &data, none));
        assert_eq!(current_step(&session, &data).unwrap().id, "build");

        // Build + win chain straight through to completion.
        session.tutorial_built = true;
        session.won = true;
        assert!(advance(&mut session, &data, none));
        assert!(current_step(&session, &data).is_none(), "tutorial finished");
        let (done, total) = progress(&session, &data);
        assert_eq!(done, total);
    }

    #[test]
    fn dismissed_tutorial_shows_nothing() {
        let (data, mut session) = boot();
        session.tutorial_dismissed = true;
        assert!(current_step(&session, &data).is_none());
        assert!(!advance(
            &mut session,
            &data,
            TutorialInputs { camera_moved: true }
        ));
    }
}

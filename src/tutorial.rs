//! The tutorial: a sequence of data-driven steps (`tutorial.json`) shown
//! in the HUD. Each step completes when the player actually does the
//! thing — pan the camera, place a building, reassign a worker, survive
//! the famine, win. Pure guidance: it reads state and never touches the sim.

use crate::data::{GameData, TutorialDone, TutorialStepDef};
use crate::simulation::{self, food};
use crate::state::creatures::Job;
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
        if !step_done(&step.done, session, data, inputs) {
            break;
        }
        session.tutorial_step += 1;
        advanced = true;
    }
    advanced
}

fn step_done(
    done: &TutorialDone,
    session: &GameSession,
    data: &GameData,
    inputs: TutorialInputs,
) -> bool {
    let sim_time = simulation::sim_seconds(session);
    match done {
        TutorialDone::CameraMoved => inputs.camera_moved,
        TutorialDone::AnyReassign => session.tutorial_reassigned,
        // The player has answered the famine: either they responded early
        // (extra carriers and a positive calorie balance), or they're past
        // the first-crisis window with the larder healthy again.
        TutorialDone::FamineRecovered { value } => {
            let responded = session.job_count(Job::Carrier) > data.balance.start_carriers as usize
                && session.economy.production_ema_per_min
                    > food::consumption_per_min(session, data);
            responded
                || (sim_time >= 330.0 && session.economy.food >= *value && !session.famine_active)
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

        // Step 2 asks for a build site; step 3 needs a reassign.
        assert!(!advance(&mut session, &data, none));
        session.tutorial_built = true;
        assert!(advance(&mut session, &data, none));
        assert_eq!(current_step(&session, &data).unwrap().id, "jobs");
        let moved = session.reassign(Job::Miner, Job::Carrier, |_| true);
        assert!(moved);
        assert!(advance(&mut session, &data, none));
        assert_eq!(current_step(&session, &data).unwrap().id, "famine");

        // Famine step clears early once the player has responded: carriers
        // above the starting crew (the reassign above did that) and a
        // positive calorie balance.
        assert!(!advance(&mut session, &data, none), "net is still negative");
        session.economy.production_ema_per_min = 999.0;
        assert!(advance(&mut session, &data, none));
        assert_eq!(current_step(&session, &data).unwrap().id, "goals");

        // Winning finishes the tutorial.
        session.won = true;
        assert!(advance(&mut session, &data, none));
        assert!(current_step(&session, &data).is_none(), "tutorial finished");
        let (done, total) = progress(&session, &data);
        assert_eq!(done, total);
    }

    #[test]
    fn famine_step_also_clears_after_recovery() {
        let (data, mut session) = boot();
        let none = TutorialInputs::default();
        session.tutorial_step = 3;
        assert_eq!(current_step(&session, &data).unwrap().id, "famine");

        // No extra carriers, no production — only riding out the crisis
        // window with a healthy larder counts.
        session.economy.food = 50.0;
        assert!(!advance(&mut session, &data, none), "too early to count");
        session.tick = (400.0 / simulation::SIM_DT) as u64;
        assert!(advance(&mut session, &data, none));
        assert_eq!(current_step(&session, &data).unwrap().id, "goals");
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

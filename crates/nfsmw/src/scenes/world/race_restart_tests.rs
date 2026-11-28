//! The pause menu's view of a race in the installed world: when a race counts as in progress, and what a restart does.
//! Spec: `docs/specs/race-restart.md`.

use blackbox_race::Phase;
use game_install::GameDir;

use super::drive::STEP;
use super::race::{RaceOptions, Stage};
use super::{DriveOptions, Options, WorldScene};
use crate::app::test_backend::NullBackend;
use crate::input::ActionState;
use crate::viewer::Scene;

fn pump(scene: &mut WorldScene, renderer: &mut NullBackend, until: impl Fn(&WorldScene) -> bool) {
    let input = ActionState::default();
    for _ in 0..6000 {
        scene.update(renderer, &input, STEP);
        if until(scene) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    panic!("the world did not get there in time");
}

fn running(scene: &WorldScene) -> bool {
    scene.race.as_ref().is_some_and(|s| s.stage == Stage::Running && s.engine.is_some())
}

#[test]
#[ignore = "needs the installed game, car physics, the road network and world collision; set NFSMW_GAME_DIR"]
fn a_restart_runs_the_race_in_progress_again_from_its_grid() {
    let dir = GameDir::open(std::env::var_os("NFSMW_GAME_DIR").expect("NFSMW_GAME_DIR")).unwrap();
    let mut scene = WorldScene::open(
        &dir,
        Options {
            at: Some([2151.0, 1399.0]),
            height: 40.0,
            heading: 45.0,
            pitch: -20.0,
            fog_distance: 1500.0,
            wait_for_load: true,
            drive: Some(DriveOptions { car: "ELISE".into(), script: None }),
        },
    )
    .unwrap();
    let mut renderer = NullBackend::new(crate::settings::test_caps::native());
    scene.init(&mut renderer).unwrap();
    pump(&mut scene, &mut renderer, |s| s.ready());
    assert!(!scene.race_in_progress(), "free roam: no race");
    assert!(!scene.restart_race_now(), "nothing to restart in free roam");

    // A sprint of the install with no opponents and no traffic, as a quick race could ask for it.
    let data = scene.gameplay.clone().expect("the install's race data");
    let sprint = data
        .races
        .iter()
        .find(|r| {
            let p = &r.params;
            r.race_type == Some(nfsmw_data::gameplay::RaceType::Sprint)
                && r.start.is_some()
                && r.finish.is_some()
                && p.do_countdown
                && !p.rolling_start
        })
        .expect("a sprint with a start, a finish and a countdown");
    let options = RaceOptions { opponents: Some(0), traffic: Some(0), ..Default::default() };
    scene.queue_race(sprint.name.clone(), options);
    assert!(scene.race_in_progress(), "a race that is asked for is staged");
    pump(&mut scene, &mut renderer, running);
    assert!(scene.race_in_progress(), "on the grid, counting down");

    // Let the race run: the countdown ends and the clock moves.
    pump(&mut scene, &mut renderer, |s| {
        s.race.as_ref().and_then(|r| r.engine.as_ref()).is_some_and(|e| e.phase() == Phase::Racing && e.elapsed() > 1.0)
    });
    let before = scene.race.as_ref().unwrap().engine.as_ref().unwrap().elapsed();
    assert!(before > 1.0);

    // Restart: the same race and options, a fresh session at the staging step, the clock back at the beginning.
    assert!(scene.restart_race_now());
    assert!(scene.race_in_progress());
    let session = scene.race.as_ref().expect("the race was started again");
    assert_eq!(session.name, sprint.name);
    assert_eq!(session.options, options);
    assert!(matches!(session.stage, Stage::Loading { .. }), "back at the loading stage until the grid is ready");
    assert!(session.engine.is_none() && session.result.is_none() && session.report.is_none());
    pump(&mut scene, &mut renderer, running);
    let engine = scene.race.as_ref().unwrap().engine.as_ref().unwrap();
    assert!(engine.elapsed() < before, "the clock restarted");
    assert!(matches!(engine.phase(), Phase::Countdown { .. }), "the countdown runs again");
    assert_eq!(engine.racers()[0].progress.reached, 0, "no checkpoint is kept");

    // Ending the race (the console's `race stop`) leaves nothing to ask about.
    assert!(scene.abandon_race());
    assert!(!scene.race_in_progress());
    assert!(!scene.restart_race_now());
}

#[test]
#[ignore = "needs installed race data and the road network; set NFSMW_GAME_DIR"]
fn career_weather_uses_the_event_and_preserves_the_dday_override_on_exit() {
    let dir = GameDir::open(std::env::var_os("NFSMW_GAME_DIR").expect("set NFSMW_GAME_DIR")).unwrap();
    let mut scene = WorldScene::open(
        &dir,
        Options {
            at: Some([2151.0, 1399.0]),
            height: 40.0,
            heading: 45.0,
            pitch: -20.0,
            fog_distance: 1500.0,
            wait_for_load: true,
            drive: Some(DriveOptions { car: "BMWM3GTR".into(), script: None }),
        },
    )
    .unwrap();
    let mut renderer = NullBackend::new(crate::settings::test_caps::native());
    scene.init(&mut renderer).unwrap();
    pump(&mut scene, &mut renderer, |s| s.ready());

    let career = RaceOptions { career: true, ..Default::default() };
    scene.start_race("16.1.0", career).unwrap();
    assert!(scene.environment_state().sun_phase > 0.9, "D-Day requests the end of the day");
    assert_eq!(scene.rain.story_override(), -1.0);
    assert!(scene.abandon_race());
    assert_eq!(scene.rain.story_override(), -1.0, "D-Day keeps the event's dry override");

    let previous_phase = scene.environment_state().sun_phase;
    scene.start_race("16.8.1", career).unwrap();
    assert_eq!(scene.environment_state().sun_phase, previous_phase, "inherited negative TOD keeps the phase");
    assert_eq!(scene.rain.story_override(), 0.0, "the meeting uses regional weather");
    assert!(scene.abandon_race());

    scene.start_race("16.1.1", career).unwrap();
    assert!((scene.environment_state().sun_phase - 0.5).abs() < 1e-5, "Rog requests midday");
    assert_eq!(scene.rain.story_override(), -1.0);
    assert!(scene.abandon_race());
    assert_eq!(scene.rain.story_override(), -1.0, "Rog is also marked as D-Day in the install");

    let data = scene.gameplay.clone().expect("the install's race data");
    let ordinary = data
        .races
        .iter()
        .find(|race| {
            race.race_type == Some(nfsmw_data::gameplay::RaceType::Sprint)
                && race.start.is_some()
                && race.finish.is_some()
                && !race.story.dday
        })
        .expect("a non-D-Day sprint");
    scene.start_race(&ordinary.name, career).unwrap();
    scene.rain.set_story_override(-1.0);
    assert!(scene.abandon_race());
    assert_eq!(scene.rain.story_override(), 0.0, "ordinary race exit restores regional weather");
}

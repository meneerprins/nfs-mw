use super::*;

#[test]
fn the_event_sequence_fires_tags_when_entering_camera_keys() {
    let mut nis = three_camera_cuts();
    assert_eq!(nis.advance(0.0), [NisEvent::CellCall(13)]);
    assert!(nis.advance(SETTLE_SECS + 0.5).is_empty());
    assert_eq!(nis.advance(0.26), [NisEvent::CellCall(11)]);
    assert!(nis.advance(0.25).is_empty(), "the camera remains on the same key");
}

#[test]
fn crossing_several_keys_selects_only_the_destination_cut() {
    let mut nis = three_camera_cuts();
    assert_eq!(nis.advance(0.0), [NisEvent::CellCall(13)]);
    assert_eq!(nis.advance(SETTLE_SECS + 1.55), [NisEvent::CellCall(13)]);
    assert!(nis.advance(0.25).is_empty(), "the skipped key does not play later");
}

fn three_camera_cuts() -> NisPlayer {
    let mut plan = plan("IntroNis05", NisKind::Intro);
    let mut camera = track(3.0, 0);
    for time in [0.25, 0.5] {
        camera.keys.push(Key { time, ..camera.keys[0].clone() });
    }
    plan.camera = Some(camera);

    let mut package = sequence();
    let action = package.actions.get_mut(&40).unwrap();
    action.tags.insert(blackbox_attrib::vlt_hash("ICECut0"), 101);
    action.tags.insert(blackbox_attrib::vlt_hash("ICECut1"), 102);
    action.tags.insert(blackbox_attrib::vlt_hash("ICECut2"), 101);
    action.times.clear();
    let mut nis = NisPlayer::default();
    nis.start_with_cars(plan, Vec::new(), Some(Arc::new(package)), Characters::default()).unwrap();

    nis
}

#[test]
#[ignore = "needs installed NIS camera tracks and event bundles; set NFSMW_GAME_DIR"]
fn installed_nis_lighting_follows_the_authored_camera_cuts() {
    use nfsmw_data::nis::{CameraLibrary, NisBundle};

    let dir = GameDir::open(std::env::var_os("NFSMW_GAME_DIR").expect("set NFSMW_GAME_DIR")).unwrap();
    let library = CameraLibrary::load(&dir).unwrap();
    for (name, expected, overcast) in [
        (
            "IntroNisDD",
            &[
                (0.08667, 0.184358, 1.0, 4.319767),
                (12.599, 0.254335, 1.0, 6.062197),
                (14.749, 0.254335, 1.0, 3.702657),
                (18.275, 0.231214, 1.0, 4.900579),
                (23.263, 0.184358, 1.0, 4.319767),
                (39.001, 0.184358, 1.0, 4.365774),
            ][..],
            0.096648,
        ),
        ("IntroNis10", &[(0.0, 0.022346, 0.0, 0.290406), (14.98667, 0.022346, 1.0, 3.28)][..], 0.13352),
    ] {
        let bundle = NisBundle::load(&dir, name).unwrap();
        let camera = library.track(name, 0).expect("the scene's camera");
        let origin = Anchor::facing(Vec3::ZERO, 0);
        let plan = Plan::new(name, &bundle.scene, Some(camera.clone()), origin, NisKind::Intro).unwrap();
        let end = plan.clock.duration();
        let mut nis = NisPlayer::default();
        nis.start_with_cars(plan, Vec::new(), bundle.sequence.clone(), Characters::default()).unwrap();

        let mut lighting = Vec::new();
        for _ in 0..18000 {
            let events = nis.advance(1.0 / 120.0);
            let time = nis.play.as_ref().map_or(end, |scene| scene.run.scene_time());
            lighting.extend(events.into_iter().filter_map(|event| match event {
                NisEvent::TimeOfDay(light) => Some((time, light)),
                _ => None,
            }));
            if !nis.is_playing() {
                break;
            }
        }
        assert!(!nis.is_playing(), "{name} completes");
        assert_eq!(lighting.len(), expected.len(), "{name} fired {lighting:?}");
        for ((observed, light), &(time, phase, rate, azimuth)) in lighting.into_iter().zip(expected) {
            assert!((observed - time).abs() <= 1.0 / 30.0, "{name} fired at {observed} s, expected {time} s");
            assert_eq!((light.phase, light.rate, light.azimuth, light.overcast), (phase, rate, azimuth, overcast));
        }
    }
}

//! The Touch controllers' mapping, end to end: the player's game library (`WO3_ENGINE_LIB`) and their disc's cue sheet
//! (`WO3_DISC`; skipped without them). On a wide floor the ship is flown at speed, then turned: a grip (an air brake)
//! held on the turn's side turns it tighter than the stick alone.
use engine_core::glam::Vec3;
use engine_core::{Applied, EngineCore, PadHeld, PlacePlayer, Stage, TouchState};
use headless_core::library::test_paths;
use wip3out_engine::stage::DEFAULT_SCALE as S;

/// Put down at rest, flown straight 40 frames, then `turning` for 20: how far its heading turned, degrees.
fn turn(engine: &mut Box<dyn EngineCore>, turning: TouchState) -> f32 {
    let start = Vec3::new(-20.0 * S, 0.0, 0.0);
    engine.place_player(PlacePlayer { point: start, forward: Vec3::X });
    for _ in 0..250 {
        engine.set_input(&TouchState::default(), PadHeld::default());
        engine.step().expect("a frame");
        if engine.focus().is_some_and(|f| (f.feet - start).length() < 0.05 * S) {
            break;
        }
    }
    let thrust = TouchState { right_trigger: 1.0, ..Default::default() };
    for _ in 0..40 {
        engine.set_input(&thrust, PadHeld::default());
        engine.step().expect("a frame");
    }
    let before = engine.focus().unwrap().forward;
    for _ in 0..20 {
        engine.set_input(&turning, PadHeld::default());
        engine.step().expect("a frame");
    }
    let after = engine.focus().unwrap().forward;
    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z).normalize();
    flat(before).angle_between(flat(after)).to_degrees()
}

#[test]
fn the_grips_are_the_air_brakes() {
    let Some([library, cue]) = test_paths(["WO3_ENGINE_LIB", "WO3_DISC"]) else { return };
    let data = std::env::temp_dir().join(format!("wo3-controls-{}", std::process::id()));
    let mut engine = wip3out_engine::engine::factory(library, cue, data.clone())().expect("the game");
    let h = 40.0 * S;
    let floor = [Vec3::new(-h, 0.0, -h), Vec3::new(-h, 0.0, h), Vec3::new(h, 0.0, h), Vec3::new(h, 0.0, -h)];
    assert_eq!(engine.set_stage(&Stage { revision: 1, room_quads: vec![floor], ..Default::default() }), Applied::Done);
    let start = std::time::Instant::now();
    while !engine.ready_to_place() {
        assert!(start.elapsed().as_secs() < 180, "no race after three minutes");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let right = TouchState { right_trigger: 1.0, left_stick: [1.0, 0.0], ..Default::default() };
    let stick = turn(&mut engine, right);
    let grip_right = turn(&mut engine, TouchState { right_grip: 1.0, ..right });
    let grip_left = turn(&mut engine, TouchState { left_grip: 1.0, ..right });
    let grip_only = turn(&mut engine, TouchState { right_trigger: 1.0, right_grip: 1.0, ..Default::default() });
    eprintln!("turned: stick {stick:.1}, + right grip {grip_right:.1}, + left grip {grip_left:.1}, right grip alone {grip_only:.1} degrees");
    assert!(grip_right > stick * 1.05, "the right air brake tightens a right turn: {grip_right} against {stick}");
    assert!(grip_left < stick * 0.95, "the left air brake widens it: {grip_left} against {stick}");
    assert!(grip_only > 2.0, "the right air brake alone turns the ship: {grip_only}");

    // (the right trigger half pulled: an analog throttle, a slower top speed than pulled fully)
    let speed = |engine: &mut Box<dyn EngineCore>, trigger: f32| {
        let start = Vec3::new(-20.0 * S, 0.0, 0.0);
        engine.place_player(PlacePlayer { point: start, forward: Vec3::X });
        for _ in 0..250 {
            engine.set_input(&TouchState::default(), PadHeld::default());
            engine.step().expect("a frame");
            if engine.focus().is_some_and(|f| (f.feet - start).length() < 0.05 * S) {
                break;
            }
        }
        let mut xs = Vec::new();
        for _ in 0..90 {
            engine.set_input(&TouchState { right_trigger: trigger, ..Default::default() }, PadHeld::default());
            engine.step().expect("a frame");
            xs.push(engine.focus().unwrap().feet.x / S);
        }
        (xs[89] - xs[69]) / (20.0 * 0.04)
    };
    let (full, half) = (speed(&mut engine, 1.0), speed(&mut engine, 0.5));
    eprintln!("top speed: trigger pulled fully {full:.2} m/s, half {half:.2} m/s");
    assert!(half < 0.8 * full && half > 0.2 * full, "half the trigger, a slower ship: {half} against {full}");
    let _ = std::fs::remove_dir_all(data);
}

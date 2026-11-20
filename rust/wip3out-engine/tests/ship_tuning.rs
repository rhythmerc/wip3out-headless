//! The ship's settings, end to end: the player's game library (`WO3_ENGINE_LIB`) and their disc's cue sheet
//! (`WO3_DISC`; skipped without them). On a 30 m floor the ship is put down at one end, facing the other, and flown
//! straight with thrust held, once a setting: twice the speed flies it faster, three times the acceleration further in
//! its first second. (Distances in real metres: the runtime's world in game metres, `DEFAULT_SCALE` a real one.)
use engine_core::glam::Vec3;
use engine_core::{Applied, EngineCore, PadHeld, PlacePlayer, Stage, TouchState};
use headless_core::library::test_paths;
use wip3out_engine::engine::settings;
use wip3out_engine::stage::DEFAULT_SCALE as S;

/// Flown from the start for `frames` frames: the distance it went in its first `early` frames and its speed (m/s)
/// over the last 20, and its feet's height and the drawn height at the end.
fn fly(engine: &mut Box<dyn EngineCore>, frames: usize, early: usize) -> (f32, f32, f32, f32) {
    let start = Vec3::new(-14.0 * S, 0.0, 0.0);
    engine.place_player(PlacePlayer { point: start, forward: Vec3::X });
    // (put down at the next frame the pad flies the ship)
    for _ in 0..250 {
        engine.set_input(&TouchState::default(), PadHeld::default());
        engine.step().expect("a frame");
        if engine.focus().is_some_and(|f| (f.feet - start).length() < 0.05 * S) {
            break;
        }
    }
    let thrust = TouchState { right_trigger: 1.0, ..Default::default() };
    let mut xs = Vec::new();
    for _ in 0..frames {
        engine.set_input(&thrust, PadHeld::default());
        engine.step().expect("a frame");
        xs.push(engine.focus().expect("the ship").feet.x / S);
    }
    let focus = engine.focus().unwrap();
    let speed = (xs[frames - 1] - xs[frames - 21]) / (20.0 * 0.04);
    (xs[early] - xs[1], speed, focus.feet.y / S, focus.height / S)
}

#[test]
fn speed_and_acceleration() {
    let Some([library, cue]) = test_paths(["WO3_ENGINE_LIB", "WO3_DISC"]) else { return };
    let data = std::env::temp_dir().join(format!("wo3-tuning-{}", std::process::id()));
    let mut engine = wip3out_engine::engine::factory(library, cue, data.clone())().expect("the game");
    let h = 15.0;
    let h = h * S;
    let floor = [Vec3::new(-h, 0.0, -h), Vec3::new(-h, 0.0, h), Vec3::new(h, 0.0, h), Vec3::new(h, 0.0, -h)];
    assert_eq!(engine.set_stage(&Stage { revision: 1, room_quads: vec![floor], ..Default::default() }), Applied::Done);
    let start = std::time::Instant::now();
    while !engine.ready_to_place() {
        assert!(start.elapsed().as_secs() < 180, "no race after three minutes");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    let (early, speed, feet, height) = fly(&mut engine, 90, 25);
    eprintln!("as the game has it: {early:.2} m in the first second, then {speed:.2} m/s; feet {feet:.3} m, drawn {height:.3} m");
    // (again, as it has it: put down at rest, the ship flies the same)
    let (early_again, speed_again, ..) = fly(&mut engine, 90, 25);
    eprintln!("again: {early_again:.2} m, {speed_again:.2} m/s");
    assert!((early_again - early).abs() < 0.1 * early.max(0.05) && (speed_again - speed).abs() < 0.05 * speed, "a run from rest is a run from rest");

    engine.set_setting(settings::SPEED, 2.0);
    let (_, fast, ..) = fly(&mut engine, 90, 25);
    eprintln!("speed 2: {fast:.2} m/s");
    assert!(fast > 1.4 * speed, "twice the top speed flies faster: {fast} against {speed} m/s");
    engine.set_setting(settings::SPEED, 1.0);

    engine.set_setting(settings::ACCELERATION, 3.0);
    let (quick, ..) = fly(&mut engine, 30, 25);
    eprintln!("acceleration 3: {quick:.2} m in the first second");
    assert!(quick > 1.3 * early, "three times the acceleration goes further at first: {quick} against {early} m");
    engine.set_setting(settings::ACCELERATION, 1.0);

    assert!(feet.abs() < 0.02 && (height - 0.078).abs() < 0.01, "a toy over the floor: feet {feet}, {height} m tall");
    let _ = std::fs::remove_dir_all(data);
}

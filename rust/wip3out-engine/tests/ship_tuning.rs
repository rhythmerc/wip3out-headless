//! The ship's settings, end to end: the player's game library (`WO3_ENGINE_LIB`) and their disc's cue sheet
//! (`WO3_DISC`; skipped without them). On a 30 m floor the ship is put down at one end, facing the other, and flown
//! straight with thrust held, once a setting: twice the speed flies it faster, three times the acceleration further in
//! its first second, twice the scale hovers it twice as high over the floor.
use engine_core::glam::Vec3;
use engine_core::{Applied, EngineCore, PadHeld, PlacePlayer, Stage, TouchState};
use headless_core::library::test_paths;
use wip3out_engine::engine::settings;

/// Flown from the start for `frames` frames: the distance it went in its first `early` frames and its speed (m/s)
/// over the last 20, and its feet's height and the drawn height at the end.
fn fly(engine: &mut Box<dyn EngineCore>, frames: usize, early: usize) -> (f32, f32, f32, f32) {
    engine.place_player(PlacePlayer { point: Vec3::new(-14.0, 0.0, 0.0), forward: Vec3::X });
    let thrust = TouchState { right_trigger: 1.0, ..Default::default() };
    let mut xs = Vec::new();
    for _ in 0..frames {
        engine.set_input(&thrust, PadHeld::default());
        engine.step().expect("a frame");
        xs.push(engine.focus().expect("the ship").feet.x);
    }
    let focus = engine.focus().unwrap();
    let speed = (xs[frames - 1] - xs[frames - 21]) / (20.0 * 0.04);
    (xs[early] - xs[1], speed, focus.feet.y, focus.height)
}

#[test]
fn speed_acceleration_and_scale() {
    let Some([library, cue]) = test_paths(["WO3_ENGINE_LIB", "WO3_DISC"]) else { return };
    let data = std::env::temp_dir().join(format!("wo3-tuning-{}", std::process::id()));
    let mut engine = wip3out_engine::engine::factory(library, cue, data.clone())().expect("the game");
    let h = 15.0;
    let floor = [Vec3::new(-h, 0.0, -h), Vec3::new(-h, 0.0, h), Vec3::new(h, 0.0, h), Vec3::new(h, 0.0, -h)];
    assert_eq!(engine.set_stage(&Stage { revision: 1, room_quads: vec![floor], ..Default::default() }), Applied::Done);
    let start = std::time::Instant::now();
    while !engine.ready_to_place() {
        assert!(start.elapsed().as_secs() < 180, "no race after three minutes");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    let (early, speed, feet, height) = fly(&mut engine, 90, 25);
    eprintln!("as the game has it: {early:.2} m in the first second, then {speed:.2} m/s; feet {feet:.3} m, drawn {height:.3} m");

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

    engine.set_setting(settings::SCALE, 2.0);
    let (_, _, feet2, height2) = fly(&mut engine, 30, 25);
    eprintln!("scale 2: feet {feet2:.3} m, drawn {height2:.3} m");
    assert!((height2 - 2.0 * height).abs() < 1e-4, "drawn twice as big");
    assert!(feet2.abs() < 0.04, "still on the floor: {feet2} m");
    let _ = std::fs::remove_dir_all(data);
}

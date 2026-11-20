//! The ship flown into a corner: the player's game library (`WO3_ENGINE_LIB`) and their disc's cue sheet
//! (`WO3_DISC`; skipped without them). A 3 m room walled all round; the ship put down near a corner, heading straight
//! into it, thrust held, then the stick held over: it stays inside both walls, on the floor, and flies out again.
use engine_core::glam::Vec3;
use engine_core::{Applied, PadHeld, PlacePlayer, Stage, TouchState};
use headless_core::library::test_paths;
use wip3out_engine::stage::DEFAULT_SCALE as S;

fn wall(a: Vec3, b: Vec3, height: f32) -> [Vec3; 4] {
    let up = Vec3::Y * height;
    let q = [a, b, b + up, a + up];
    let n = (q[1] - q[0]).cross(q[2] - q[0]);
    // (its front to the room's middle)
    if n.dot(-a) > 0.0 { q } else { [q[3], q[2], q[1], q[0]] }
}

#[test]
fn into_a_corner_and_out() {
    let Some([library, cue]) = test_paths(["WO3_ENGINE_LIB", "WO3_DISC"]) else { return };
    let data = std::env::temp_dir().join(format!("wo3-corner-{}", std::process::id()));
    let mut engine = wip3out_engine::engine::factory(library, cue, data.clone())().expect("the game");
    let h = 1.5 * S;
    let c = |x: f32, z: f32| Vec3::new(x * h, 0.0, z * h);
    let corners = [c(-1.0, -1.0), c(1.0, -1.0), c(1.0, 1.0), c(-1.0, 1.0)];
    let mut quads = vec![[corners[0], corners[3], corners[2], corners[1]]];
    for i in 0..4 {
        quads.push(wall(corners[i], corners[(i + 1) % 4], 0.3 * S));
    }
    assert_eq!(engine.set_stage(&Stage { revision: 1, room_quads: quads, ..Default::default() }), Applied::Done);
    while !engine.ready_to_place() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // (0.7 m from the corner at +x +z, along its diagonal, facing it)
    let start = Vec3::new(0.8, 0.0, 0.8) * S;
    engine.place_player(PlacePlayer { point: start, forward: Vec3::new(1.0, 0.0, 1.0).normalize() });
    for _ in 0..250 {
        engine.set_input(&TouchState::default(), PadHeld::default());
        engine.step().expect("a frame");
        if engine.focus().is_some_and(|f| (f.feet - start).length() < 0.05 * S) {
            break;
        }
    }
    let thrust = TouchState { right_trigger: 1.0, ..Default::default() };
    let turn = TouchState { right_trigger: 1.0, left_stick: [-1.0, 0.0], ..Default::default() };
    let (mut furthest, mut lowest, mut highest) = (0f32, f32::MAX, f32::MIN);
    let mut later = Vec::new();
    for frame in 0..400 {
        engine.set_input(if frame < 150 { &thrust } else { &turn }, PadHeld::default());
        engine.step().expect("a frame");
        let feet = engine.focus().expect("the ship").feet / S;
        furthest = furthest.max(feet.x.abs()).max(feet.z.abs());
        if frame > 10 {
            lowest = lowest.min(feet.y);
            highest = highest.max(feet.y);
        }
        if frame >= 300 {
            later.push(feet);
        }
    }
    let moved: f32 = later.windows(2).map(|w| (w[1] - w[0]).length()).sum();
    eprintln!("furthest out {furthest:.3} m of 1.5; feet between {lowest:.3} and {highest:.3} m; flown {moved:.2} m in the last 4 s");
    // (the push-out lets the ship centre about 0.04 m past a wall, a frame at a time, in a corner too)
    assert!(furthest < 1.5 + 0.06, "inside both walls of the corner: {furthest} m");
    assert!(lowest > -0.02 && highest < 0.05, "on the floor: {lowest}..{highest} m");
    assert!(moved > 1.0, "out of the corner and flying: {moved} m");
    let _ = std::fs::remove_dir_all(data);
}

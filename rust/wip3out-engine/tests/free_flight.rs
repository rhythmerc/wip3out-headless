//! The ship flying over a room, end to end through the engine contract: the player's game library (`WO3_ENGINE_LIB`)
//! and their disc's cue sheet (`WO3_DISC`; skipped without them). A 6 m floor walled all round; the ship put down in
//! the middle facing +x, thrust held, then steered: it hovers over the floor all the while, never off it (the game's
//! flying flag never set), and stays inside the walls.
use engine_core::glam::Vec3;
use engine_core::{Applied, PadHeld, PlacePlayer, Stage, TouchState};
use headless_core::library::test_paths;

/// A wall from `a` to `b` on the floor, `height` tall, its front (corners counter-clockwise) to `inside`.
fn wall(a: Vec3, b: Vec3, height: f32, inside: Vec3) -> [Vec3; 4] {
    let up = Vec3::Y * height;
    let q = [a, b, b + up, a + up];
    let n = (q[1] - q[0]).cross(q[2] - q[0]);
    if n.dot(inside - a) > 0.0 { q } else { [q[3], q[2], q[1], q[0]] }
}

fn room(half: f32) -> Stage {
    let c = |x: f32, z: f32| Vec3::new(x * half, 0.0, z * half);
    let corners = [c(-1.0, -1.0), c(1.0, -1.0), c(1.0, 1.0), c(-1.0, 1.0)];
    let mut quads = vec![[corners[0], corners[3], corners[2], corners[1]]];
    for i in 0..4 {
        quads.push(wall(corners[i], corners[(i + 1) % 4], 0.3, Vec3::ZERO));
    }
    Stage { revision: 1, room_quads: quads, ..Default::default() }
}

#[test]
fn the_ship_flies_over_the_room_and_stays_in_its_walls() {
    let Some([library, cue]) = test_paths(["WO3_ENGINE_LIB", "WO3_DISC"]) else { return };
    let data = std::env::temp_dir().join(format!("wo3-free-flight-{}", std::process::id()));
    let mut engine = wip3out_engine::engine::factory(library, cue, data.clone())().expect("the game");

    let half = 3.0;
    let stage = room(half);
    assert_eq!(engine.set_stage(&stage), Applied::Done);
    // (the race: its menus driven by the library, some 25 seconds)
    let start = std::time::Instant::now();
    while !engine.ready_to_place() {
        assert!(start.elapsed().as_secs() < 180, "no race after three minutes");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    engine.place_player(PlacePlayer { point: Vec3::ZERO, forward: Vec3::X });

    let thrust = TouchState { right_trigger: 1.0, ..Default::default() };
    let steer = TouchState { right_trigger: 1.0, left_stick: [-1.0, 0.0], ..Default::default() };
    let (mut lowest, mut highest, mut travelled, mut furthest) = (f32::MAX, f32::MIN, 0.0, 0.0f32);
    let mut last: Option<Vec3> = None;
    for frame in 0..300 {
        engine.set_input(if frame < 150 { &thrust } else { &steer }, PadHeld::default());
        engine.step().expect("a frame");
        let focus = engine.focus().expect("the ship");
        let feet = focus.feet;
        if frame == 1 {
            assert!(feet.length() < 0.05, "put down where asked: {feet}");
            assert!(focus.forward.dot(Vec3::X) > 0.95, "facing +x: {}", focus.forward);
        }
        if frame >= 10 {
            lowest = lowest.min(feet.y);
            highest = highest.max(feet.y);
        }
        furthest = furthest.max(feet.x.abs()).max(feet.z.abs());
        if let Some(l) = last {
            travelled += (feet - l).length();
        }
        last = Some(feet);
        if frame % 25 == 0 {
            eprintln!("frame {frame}: feet {feet:.3} forward {:.2}", focus.forward);
        }
    }
    eprintln!("travelled {travelled:.2} m; feet between {lowest:.3} and {highest:.3} m; furthest out {furthest:.3} m");
    assert!(travelled > 5.0, "it flew: {travelled} m");
    // (a centimetre is 40 units. The game holds the ship at its hover; knocked about by the walls it rides up a few
    // centimetres, as it does off the circuit's own walls)
    assert!(lowest > -0.02 && highest < 0.05, "it hovered over the floor: {lowest}..{highest} m");
    // (its centre within the walls: hitting one hard, it may slip a few centimetres past its plane for a frame before
    // the game, or the library's push-out, puts it back)
    assert!(furthest < half + 0.05, "it stayed in the walls: {furthest} m");
    let _ = std::fs::remove_dir_all(data);
}

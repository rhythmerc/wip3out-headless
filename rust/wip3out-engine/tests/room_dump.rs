//! A real room as the app gave it to the engine (its `dump_stage` dev flag), replayed: the player's game library
//! (`WO3_ENGINE_LIB`), their disc's cue sheet (`WO3_DISC`) and the dump (`WO3_STAGE_DUMP`; skipped without them - a
//! room's scan is the player's, kept out of the repository). The ship is put down where the player put it on the
//! headset, on the floor and on furniture, at the dump's world scale and at the engine's natural one: left alone it
//! stays where it was put, upright, and flown it stays near.
use engine_core::glam::Vec3;
use engine_core::{Applied, EngineCore, PadHeld, PlacePlayer, Stage, TouchState};
use headless_core::library::test_paths;

mod common;
use common::{read_dump, scaled};

/// Where the player put the ship down on the headset (the app's log; stage coordinates at the dump's scale): on the
/// floor, on the bed's top, and on a scanned lump on the bed (a pillow, about 1 m (stage) over the bed's top).
const PLACES: [[f32; 3]; 6] =
    [[-2.08, 0.01, 5.81], [2.62, 0.01, 10.88], [-7.30, 0.01, 8.72], [0.11, 0.01, 2.94], [-3.42, 1.44, -1.87], [-4.32, 2.40, -4.34]];
/// The one on the lump: it may slide off it (as a ship does down a slope), onto the bed's top (1.44, the dump's scale).
const ON_A_LUMP: usize = 5;
const BED_TOP: f32 = 1.44;

/// Steps the engine with the controllers as `touch`, `frames` times; the ship's feet and up after each. Halfway, the
/// stage given again as a new revision (the app rebuilds it as the scan grows).
fn fly(engine: &mut dyn EngineCore, touch: &TouchState, frames: usize, scale: f32, stage: &Stage, revision: &mut u64) -> Vec<(Vec3, Vec3)> {
    (0..frames)
        .map(|n| {
            if n == frames / 2 {
                *revision += 1;
                let mut again = stage.clone();
                again.revision = *revision;
                assert_eq!(engine.set_stage(&again), Applied::Done);
            }
            engine.set_world_scale(scale);
            engine.set_input(touch, PadHeld::default());
            let start = std::time::Instant::now();
            engine.step().expect("a frame");
            STEP_TIME.with(|t| t.set(t.get() + start.elapsed()));
            STEPS.with(|s| s.set(s.get() + 1));
            let f = engine.focus().expect("the ship");
            (f.feet, f.up)
        })
        .collect()
}

thread_local! {
    static STEP_TIME: std::cell::Cell<std::time::Duration> = const { std::cell::Cell::new(std::time::Duration::ZERO) };
    static STEPS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// The ship put down at each of the player's places at the world scale `scale` (the stage shown `k` times as big as
/// dumped), left alone 4 s, then flown 2 s: what went wrong, each.
fn places(engine: &mut dyn EngineCore, stage: &Stage, scale: f32, k: f32, revision: &mut u64) -> Vec<String> {
    *revision += 1;
    let stage = scaled(stage, k, *revision);
    engine.set_world_scale(scale);
    assert_eq!(engine.set_stage(&stage), Applied::Done);
    // (the room's walls, as the scene has them: the ship stays between them)
    let (lo, hi) = stage.room_quads.iter().flatten().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
    let only = std::env::var("WO3_PLACE").ok().and_then(|v| v.parse::<usize>().ok());
    let trace = std::env::var_os("WO3_TRACE").is_some();
    let mut failures = Vec::new();
    let thrust = TouchState { right_trigger: 1.0, ..Default::default() };
    for (i, place) in PLACES.iter().enumerate() {
        if only.is_some_and(|o| o != i) {
            continue;
        }
        let point = Vec3::from_array(*place) * k;
        for forward in [Vec3::NEG_Z, Vec3::new(1.0, 0.0, 0.3).normalize()] {
            engine.place_player(PlacePlayer { point, forward });
            let still = fly(engine, &TouchState::default(), 100, scale, &stage, revision);
            if trace {
                for (n, (f, u)) in still.iter().enumerate() {
                    eprintln!("  {n}: feet {f:.2} up {u:.2}");
                }
            }
            let (feet, up) = *still.last().unwrap();
            let drift = still.iter().map(|(f, _)| (*f - point).length()).fold(0.0, f32::max) / scale;
            let tilt = still.iter().map(|(_, u)| u.angle_between(Vec3::Y).to_degrees()).fold(0.0, f32::max);
            let flown = fly(engine, &thrust, 50, scale, &stage, revision);
            if trace {
                for (n, (f, u)) in flown.iter().enumerate() {
                    eprintln!("  flown {n}: feet {f:.2} up {u:.2}");
                }
            }
            // (its feet - its centre less its hover - dip below the floor as it lands from a fall, its hover pressed
            // down; its body stays over it: a (game) metre is its hover, less its underside)
            let inside = |f: &Vec3| f.x > lo.x && f.x < hi.x && f.z > lo.z && f.z < hi.z && f.y > lo.y - 1.0;
            let out = flown.iter().filter(|(f, _)| !inside(f)).count();
            let outside = flown
                .iter()
                .map(|(f, _)| [lo.x - f.x, f.x - hi.x, lo.z - f.z, f.z - hi.z, lo.y - 1.0 - f.y].into_iter().fold(0.0, f32::max))
                .fold(0.0, f32::max)
                / scale;
            let jump = flown.windows(2).map(|w| (w[1].0 - w[0].0).length()).fold(0.0, f32::max) / scale;
            let line = format!(
                "x{scale} place {i} {point:.2} facing {forward:.2}: left alone 4 s, drifted at most {drift:.3} m (real), \
                 tilted at most {tilt:.0} deg, ends {feet:.2} up {up:.2}; flown 2 s, {out} frames out of the room (by \
                 {outside:.3} m), at most {jump:.3} m (real) a frame"
            );
            eprintln!("{line}");
            let stayed = if i == ON_A_LUMP {
                // (off the lump at most: still on the bed, upright)
                feet.y > (BED_TOP * k) - 0.02 * scale && up.y > 0.95
            } else {
                drift < 0.03 && tilt < 5.0
            };
            // (a frame's flight at full speed is some 0.25 m (real) at 7x; a crash into a wall at it - the game's
            // own bounce - throws the ship back up to about its length, 5.2 m (game) long)
            let length = 5.2 / scale;
            if !stayed || out > 0 || jump > 1.5 * length {
                failures.push(line);
            }
        }
    }
    failures
}

/// (the game library boots once a process: one test, both scales)
#[test]
fn the_ship_stays_where_it_is_put_in_a_real_room() {
    let Some([library, cue, dump]) = test_paths(["WO3_ENGINE_LIB", "WO3_DISC", "WO3_STAGE_DUMP"]) else { return };
    let (dumped_scale, stage) = read_dump(&std::fs::read_to_string(dump).expect("the dump"));
    assert!(dumped_scale > 0.0 && !stage.room_triangles.is_empty(), "a dump");
    let data = std::env::temp_dir().join(format!("wo3-room-dump-{}", std::process::id()));
    let mut engine = wip3out_engine::engine::factory(library, cue, data.clone())().expect("the game");
    engine.set_world_scale(dumped_scale);
    assert_eq!(engine.set_stage(&stage), Applied::Done);
    while !engine.ready_to_place() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // (as dumped - the app's 7x - then at the engine's natural scale, the room shown that much bigger)
    let natural = wip3out_engine::INFO.natural_scale;
    let mut revision = 2;
    let mut failures = places(engine.as_mut(), &stage, dumped_scale, 1.0, &mut revision);
    failures.extend(places(engine.as_mut(), &stage, natural, natural / dumped_scale, &mut revision));
    let (time, steps) = (STEP_TIME.with(|t| t.get()), STEPS.with(|s| s.get()));
    eprintln!("{steps} steps back to back, {:.2} ms a step on average (this machine, {} scanned triangles)", time.as_secs_f64() * 1000.0 / f64::from(steps), stage.room_triangles.len());
    // (as the app steps it: a frame of its own between steps - here 30 ms, a step falling due every 40 ms - the game's
    // frame running meanwhile; what a step holds the caller up)
    let mut held = std::time::Duration::ZERO;
    for _ in 0..100 {
        std::thread::sleep(std::time::Duration::from_millis(30));
        engine.set_input(&TouchState { right_trigger: 1.0, ..Default::default() }, PadHeld::default());
        let start = std::time::Instant::now();
        engine.step().expect("a frame");
        held += start.elapsed();
    }
    eprintln!("stepped between the caller's own 30 ms frames: {:.2} ms a step held up", held.as_secs_f64() * 10.0);
    let _ = std::fs::remove_dir_all(data);
    assert!(failures.is_empty(), "{failures:#?}");
}

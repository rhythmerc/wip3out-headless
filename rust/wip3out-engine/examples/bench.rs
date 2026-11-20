//! What the game costs: the time from the engine's factory to its race, and a frame's time, flown over a walled room.
//! `bench <library> <cue> [frames]` - on a desktop or on a headset (an Android build run from a shell), the player's
//! own game library and disc. The frames are stepped whole, back to back (WO3_STEP_WHOLE is set): each is the game's
//! own frame.
use engine_core::glam::Vec3;
use engine_core::{PadHeld, PlacePlayer, Stage, TouchState};
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(library), Some(cue)) = (args.get(1), args.get(2)) else {
        eprintln!("bench <library> <cue> [frames]");
        std::process::exit(2);
    };
    let frames: usize = args.get(3).and_then(|f| f.parse().ok()).unwrap_or(500);
    // SAFETY: set before any thread of this process reads the environment.
    unsafe { std::env::set_var("WO3_STEP_WHOLE", "1") };
    let data = std::env::temp_dir().join(format!("wo3-bench-{}", std::process::id()));
    let start = Instant::now();
    let mut engine = wip3out_engine::engine::factory(library.into(), cue.into(), data.clone())().expect("the game");
    let made = start.elapsed();
    while !engine.ready_to_place() {
        std::thread::sleep(Duration::from_millis(10));
    }
    eprintln!("bench: made in {:.2} s, racing at {:.2} s", made.as_secs_f64(), start.elapsed().as_secs_f64());

    // (an 8 m room at 26x, walled in both windings)
    let s = 26.0;
    let h = 4.0 * s;
    let c = |x: f32, z: f32| Vec3::new(x * h, 0.0, z * h);
    let k = [c(-1.0, -1.0), c(1.0, -1.0), c(1.0, 1.0), c(-1.0, 1.0)];
    let mut quads = vec![[k[0], k[3], k[2], k[1]]];
    for i in 0..4 {
        let (a, b) = (k[i], k[(i + 1) % 4]);
        let up = Vec3::Y * 0.5 * s;
        quads.push([a, b, b + up, a + up]);
        quads.push([a + up, b + up, b, a]);
    }
    engine.set_world_scale(s);
    engine.set_stage(&Stage { revision: 1, room_quads: quads, ..Default::default() });
    engine.place_player(PlacePlayer { point: Vec3::ZERO, forward: Vec3::NEG_Z });

    let mut times = Vec::with_capacity(frames);
    let (mut flown, mut last) = (0.0f32, None::<Vec3>);
    for n in 0..frames {
        // (round and round: thrust, the stick over every other second)
        let turn = if (n / 25) % 2 == 0 { -1.0 } else { 0.0 };
        engine.set_input(&TouchState { right_trigger: 1.0, left_stick: [turn, 0.0], ..Default::default() }, PadHeld::default());
        let t = Instant::now();
        engine.step().expect("a frame");
        times.push(t.elapsed());
        if let Some(f) = engine.focus() {
            if let Some(l) = last {
                flown += (f.feet - l).length();
            }
            last = Some(f.feet);
        }
    }
    eprintln!("bench: flown {:.1} m (real), ends {:.2}", flown / s, last.unwrap_or_default() / s);
    times.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    let mean = times.iter().sum::<Duration>() / frames as u32;
    eprintln!(
        "bench: {frames} frames, mean {:.2} ms, median {:.2}, 90% {:.2}, 99% {:.2}, max {:.2} (the budget 40)",
        ms(mean),
        ms(times[frames / 2]),
        ms(times[frames * 9 / 10]),
        ms(times[frames * 99 / 100]),
        ms(times[frames - 1])
    );
    let _ = std::fs::remove_dir_all(data);
    std::process::exit(0);
}

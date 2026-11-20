//! The ship over ramps, slopes, steps and furniture: the player's game library (`WO3_ENGINE_LIB`) and their disc's cue
//! sheet (`WO3_DISC`; skipped without them). Synthetic stages at the app's living-room scale (11.3 stage metres a real
//! one), each built in both windings as a scan is, then - with `WO3_RAMP_DUMP`, a living room the app dumped with its
//! placed ramps (the player's, kept out of the repository) - its ramps ridden.
//!
//! Ridden at speed off a ramp the ship launches (several frames clear of the ground) and lands clean: never more than
//! some 30 degrees off upright, back on the ground and flying on along its way after, no frame's jump longer than the
//! ship. Up a long slope it climbs and stays on top; a low step it hovers over; a tall one stops it.
use engine_core::glam::Vec3;
use engine_core::{Applied, EngineCore, PadHeld, PlacePlayer, Stage, TouchState};
use headless_core::library::test_paths;

mod common;
use common::read_dump;

/// The living room's scale (stage metres a real one).
const SCALE: f32 = 11.3;
/// The ship's length (game metres, so stage metres), and how far over the ground counts as in the air.
const SHIP: f32 = 5.2;
const AIR: f32 = 0.5;

/// A quad in both windings.
fn both(q: [Vec3; 4], out: &mut Vec<[Vec3; 4]>) {
    out.push(q);
    out.push([q[3], q[2], q[1], q[0]]);
}

/// A box from `lo` to `hi`, its faces in both windings.
fn cuboid(lo: Vec3, hi: Vec3, out: &mut Vec<[Vec3; 4]>) {
    let p = |x: bool, y: bool, z: bool| Vec3::new(if x { hi.x } else { lo.x }, if y { hi.y } else { lo.y }, if z { hi.z } else { lo.z });
    for q in [
        [p(false, true, false), p(true, true, false), p(true, true, true), p(false, true, true)],
        [p(false, false, false), p(true, false, false), p(true, true, false), p(false, true, false)],
        [p(false, false, true), p(true, false, true), p(true, true, true), p(false, true, true)],
        [p(false, false, false), p(false, false, true), p(false, true, true), p(false, true, false)],
        [p(true, false, false), p(true, false, true), p(true, true, true), p(true, true, false)],
    ] {
        both(q, out);
    }
}

/// A ramp up along -z from z = `z0`: `profile` gives its height at a share of its `length` (0..1), `width` across x
/// at `x`, its sides and its back (a vertical face at its top end) closed.
fn ramp(x: f32, z0: f32, width: f32, length: f32, profile: impl Fn(f32) -> f32, out: &mut Vec<[Vec3; 4]>) {
    let n = 12;
    let at = |i: usize| (z0 - length * i as f32 / n as f32, profile(i as f32 / n as f32));
    let (l, r) = (x - width / 2.0, x + width / 2.0);
    for i in 0..n {
        let ((za, ya), (zb, yb)) = (at(i), at(i + 1));
        both([Vec3::new(l, ya, za), Vec3::new(r, ya, za), Vec3::new(r, yb, zb), Vec3::new(l, yb, zb)], out);
        for s in [l, r] {
            both([Vec3::new(s, 0.0, za), Vec3::new(s, 0.0, zb), Vec3::new(s, yb, zb), Vec3::new(s, ya, za)], out);
        }
    }
    let (zt, yt) = at(n);
    both([Vec3::new(l, 0.0, zt), Vec3::new(r, 0.0, zt), Vec3::new(r, yt, zt), Vec3::new(l, yt, zt)], out);
}

/// A floor 120 x 120 stage metres, walled.
fn room(extra: Vec<[Vec3; 4]>) -> Stage {
    let h = 60.0;
    let c = |x: f32, z: f32| Vec3::new(x * h, 0.0, z * h);
    let k = [c(-1.0, -1.0), c(1.0, -1.0), c(1.0, 1.0), c(-1.0, 1.0)];
    let mut quads = Vec::new();
    both([k[0], k[3], k[2], k[1]], &mut quads);
    for i in 0..4 {
        let (a, b) = (k[i], k[(i + 1) % 4]);
        both([a, b, b + Vec3::Y * 10.0, a + Vec3::Y * 10.0], &mut quads);
    }
    quads.extend(extra);
    Stage { revision: 0, room_quads: quads, ..Default::default() }
}

/// The ground under (x, z), at or below `y` plus a little: the highest upward face there.
fn ground(stage: &Stage, p: Vec3) -> f32 {
    let mut best = f32::MIN;
    for [a, b, c] in stage.triangles() {
        let n = (b - a).cross(c - a);
        if n.y.abs() < 0.6 * n.length() {
            continue;
        }
        let side = |p: Vec3, a: Vec3, b: Vec3| (b.x - a.x) * (p.z - a.z) - (b.z - a.z) * (p.x - a.x);
        let (s0, s1, s2) = (side(p, a, b), side(p, b, c), side(p, c, a));
        if (s0 < 0.0 || s1 < 0.0 || s2 < 0.0) && (s0 > 0.0 || s1 > 0.0 || s2 > 0.0) {
            continue;
        }
        let y = a.y - (n.x * (p.x - a.x) + n.z * (p.z - a.z)) / n.y;
        if y <= p.y + 1.0 && y > best {
            best = y;
        }
    }
    best
}

struct Run {
    feet: Vec<Vec3>,
    up: Vec<Vec3>,
    over: Vec<f32>,
}

fn fly(engine: &mut dyn EngineCore, stage: &Stage, place: PlacePlayer, thrust: &[f32]) -> Run {
    engine.place_player(place);
    let mut run = Run { feet: Vec::new(), up: Vec::new(), over: Vec::new() };
    for n in 0..20 + thrust.len() {
        let t = if n < 20 { 0.0 } else { thrust[n - 20] };
        engine.set_world_scale(SCALE);
        engine.set_input(&TouchState { right_trigger: t, ..Default::default() }, PadHeld::default());
        engine.step().expect("a frame");
        let f = engine.focus().expect("the ship");
        if n >= 20 {
            run.over.push(f.feet.y - ground(stage, f.feet));
            run.feet.push(f.feet);
            run.up.push(f.up);
        }
    }
    run
}

/// What a run did, and what went wrong with it for `want`.
#[derive(Clone, Copy, PartialEq)]
enum Want {
    /// Off the ramp into the air, and a clean landing.
    Launch,
    /// Up and on top of it, at `top` (stage metres), still there at the end.
    OnTop(f32),
    /// Over it, never in the air (a low step).
    Over,
    /// Stopped by it, short of `z` (a tall step: a wall).
    Stopped(f32),
    /// Upright and in the room, nothing more (a slow climb).
    Upright,
    /// Off the ramp into the air, upright (the landing: wherever the room has it).
    Air,
}

fn judge(name: &str, run: &Run, start: Vec3, dir: Vec3, want: Want) -> Option<String> {
    let low_up = run.up.iter().map(|u| u.y).fold(1.0, f32::min);
    let along: Vec<f32> = run.feet.iter().map(|f| (*f - start).dot(dir)).collect();
    let jump = run.feet.windows(2).map(|w| (w[1] - w[0]).length()).fold(0.0, f32::max);
    // (the longest run of frames in the air, and where it ended)
    let (mut air, mut best, mut end) = (0, 0, 0);
    for (n, o) in run.over.iter().enumerate() {
        air = if *o > AIR { air + 1 } else { 0 };
        if air > best {
            best = air;
            end = n;
        }
    }
    let top = run.feet.iter().map(|f| f.y).fold(f32::MIN, f32::max);
    let last = *run.feet.last().unwrap();
    let line = format!(
        "{name}: in the air {best} frames (to {end}), top {top:.2}, up.y at least {low_up:.2}, along {:.1}, most a frame {jump:.2}, ends {last:.2} {:.2} over the ground",
        along.last().unwrap(),
        run.over.last().unwrap()
    );
    eprintln!("{line}");
    if std::env::var_os("WO3_TRACE").is_some() {
        for (n, ((f, u), o)) in run.feet.iter().zip(&run.up).zip(&run.over).enumerate() {
            eprintln!("  {n}: feet {f:.2} up {u:.2} over {o:.2}");
        }
    }
    let upright = low_up > 0.85 && jump < SHIP;
    let ok = upright
        && match want {
            Want::Launch => {
                // (landed: the frames after back near the ground, flying on along its way)
                let after = &run.over[(end + 1).min(run.over.len())..];
                let settled = after.len() >= 15 && after[after.len().saturating_sub(10)..].iter().all(|o| o.abs() < AIR);
                best >= 3 && settled && along[along.len() - 1] > along[end] + SHIP
            }
            Want::OnTop(y) => (last.y - y).abs() < AIR && run.over.last().unwrap().abs() < AIR,
            Want::Over => best == 0 && along.last().unwrap() > &40.0,
            Want::Stopped(z) => run.feet.iter().all(|f| f.z > z),
            Want::Upright => true,
            Want::Air => best >= 3,
        };
    (!ok).then_some(line)
}

#[test]
fn ramps_slopes_steps_and_furniture() {
    let Some([library, cue]) = test_paths(["WO3_ENGINE_LIB", "WO3_DISC"]) else { return };
    let data = std::env::temp_dir().join(format!("wo3-ramp-{}", std::process::id()));
    let mut engine = wip3out_engine::engine::factory(library, cue, data.clone())().expect("the game");
    engine.set_world_scale(SCALE);
    while !engine.ready_to_place() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let only = std::env::var("WO3_ONLY").ok();
    let full = vec![1.0; 110];
    let slow: Vec<f32> = (0..150).map(|n| if n < 12 { 1.0 } else { 0.0 }).collect();
    let up_z = Vec3::NEG_Z;
    let mut revision = 0;
    let mut failures = Vec::new();
    let mut ride = |engine: &mut dyn EngineCore, name: &str, stage: &Stage, start: Vec3, dir: Vec3, thrust: &[f32], want: Want| {
        if only.as_deref().is_some_and(|o| !name.starts_with(o)) {
            return;
        }
        revision += 1;
        let mut stage = stage.clone();
        stage.revision = revision;
        assert_eq!(engine.set_stage(&stage), Applied::Done);
        let run = fly(engine, &stage, PlacePlayer { point: start, forward: dir }, thrust);
        failures.extend(judge(name, &run, start, dir, want));
    };

    // A wedge up to a platform 1.7 m (stage) high, off its end: as the living room's wooden one.
    let mut q = Vec::new();
    ramp(0.0, 0.0, 4.5, 5.0, |s| 1.7 * s, &mut q);
    cuboid(Vec3::new(-2.25, 0.0, -9.5), Vec3::new(2.25, 1.7, -5.0), &mut q);
    let wedge = room(q);
    ride(engine.as_mut(), "wedge, full", &wedge, Vec3::new(0.0, 0.0, 12.0), up_z, &full, Want::Launch);
    ride(engine.as_mut(), "wedge, slow", &wedge, Vec3::new(0.0, 0.0, 6.0), up_z, &slow, Want::Upright);
    // (at it 35 degrees off its line: up its side's corner, or off its side)
    let slant = Vec3::new(35f32.to_radians().sin(), 0.0, -35f32.to_radians().cos());
    ride(engine.as_mut(), "wedge, slanted", &wedge, Vec3::new(-1.5, 0.0, 2.0) - slant * 10.0, slant, &full, Want::Upright);
    // A kicker: curved up to 0.9, narrower than the ship.
    let mut q = Vec::new();
    ramp(0.0, 0.0, 2.3, 3.5, |s| 0.9 * s * s, &mut q);
    let kicker = room(q);
    ride(engine.as_mut(), "kicker, full", &kicker, Vec3::new(0.0, 0.0, 12.0), up_z, &full, Want::Launch);
    ride(engine.as_mut(), "kicker, slow", &kicker, Vec3::new(0.0, 0.0, 6.0), up_z, &slow, Want::Upright);
    // A long slope (15 degrees) up to a plateau 3 high, to stay on.
    let mut q = Vec::new();
    let rise = 3.0;
    let run = rise / 15f32.to_radians().tan();
    ramp(0.0, 0.0, 8.0, run, |s| rise * s, &mut q);
    cuboid(Vec3::new(-4.0, 0.0, -59.0), Vec3::new(4.0, rise, -run), &mut q);
    let half = vec![0.6; 110];
    ride(engine.as_mut(), "slope, on top", &room(q), Vec3::new(0.0, 0.0, 10.0), up_z, &half, Want::OnTop(rise));
    // A low step (0.15: the ship hovers over it) and a tall one (1.5: a wall), across the way.
    let mut q = Vec::new();
    cuboid(Vec3::new(-20.0, 0.0, -50.0), Vec3::new(20.0, 0.15, 0.0), &mut q);
    ride(engine.as_mut(), "low step", &room(q), Vec3::new(0.0, 0.0, 10.0), up_z, &half, Want::Over);
    let mut q = Vec::new();
    cuboid(Vec3::new(-20.0, 0.0, -50.0), Vec3::new(20.0, 1.5, 0.0), &mut q);
    ride(engine.as_mut(), "tall step", &room(q), Vec3::new(0.0, 0.0, 10.0), up_z, &full, Want::Stopped(-0.5));
    // A table top (2.5 high), the ship put down on it and flown off its edge.
    let mut q = Vec::new();
    cuboid(Vec3::new(-6.0, 0.0, -6.0), Vec3::new(6.0, 2.5, 6.0), &mut q);
    ride(engine.as_mut(), "table top", &room(q), Vec3::new(0.0, 2.5, 4.0), up_z, &full, Want::Launch);

    // The living room's own ramps, as the app dumped them.
    if let Some(dump) = std::env::var_os("WO3_RAMP_DUMP") {
        let (_, room) = read_dump(&std::fs::read_to_string(dump).expect("the dump"));
        // (the wooden wedge and platform, and the second kicker - the first is against a wall: ridden, it is hit)
        let wedge_dir = Vec3::new(0.519, 0.0, -0.855);
        let wedge_foot = Vec3::new(-37.09, -1.65, 12.885);
        ride(engine.as_mut(), "room wedge, full", &room, wedge_foot - wedge_dir * 9.0, wedge_dir, &full, Want::Launch);
        ride(engine.as_mut(), "room wedge, slow", &room, wedge_foot - wedge_dir * 6.0, wedge_dir, &slow, Want::Upright);
        let kick_dir = Vec3::new(-0.82, 0.0, -0.57).normalize();
        let kick = Vec3::new(-53.2, -1.65, 16.5);
        // (and the kicker's landing has furniture a few metres on)
        ride(engine.as_mut(), "room kicker, full", &room, kick - kick_dir * 9.0, kick_dir, &full, Want::Air);
    }
    let _ = std::fs::remove_dir_all(data);
    assert!(failures.is_empty(), "{failures:#?}");
}

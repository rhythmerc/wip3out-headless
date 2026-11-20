//! What the game's tests share: a room the app dumped (its `dump_stage` dev flag) read back, and the engine flown.
#![allow(dead_code)]
use engine_core::glam::Vec3;
use engine_core::Stage;

/// A dumped stage: its scale and the stage.
pub fn read_dump(text: &str) -> (f32, Stage) {
    let mut scale = 0.0;
    let mut stage = Stage { revision: 1, ..Default::default() };
    let mut solids: Vec<engine_core::stage::Solid> = Vec::new();
    for line in text.lines() {
        let mut words = line.split_whitespace();
        let kind = words.next().unwrap_or("");
        let id = if kind == "s" { words.next().and_then(|w| w.parse::<u64>().ok()) } else { None };
        let n: Vec<f32> = words.filter_map(|w| w.parse().ok()).collect();
        let p = |i: usize| Vec3::new(n[3 * i], n[3 * i + 1], n[3 * i + 2]);
        match kind {
            "scale" => scale = n[0],
            "q" => stage.room_quads.push([p(0), p(1), p(2), p(3)]),
            "t" => stage.room_triangles.push([p(0), p(1), p(2)]),
            "s" => {
                let id = id.unwrap_or(0);
                let quad = [p(0), p(1), p(2), p(3)];
                match solids.iter_mut().find(|s| s.id == id) {
                    Some(s) => s.quads.push(quad),
                    None => solids.push(engine_core::stage::Solid { id, quads: vec![quad] }),
                }
            }
            _ => {}
        }
    }
    stage.solids = solids;
    (scale, stage)
}

/// The stage shown `k` times bigger.
pub fn scaled(stage: &Stage, k: f32, revision: u64) -> Stage {
    let mut s = stage.clone();
    s.revision = revision;
    s.room_quads.iter_mut().flatten().for_each(|p| *p *= k);
    s.room_triangles.iter_mut().flatten().for_each(|p| *p *= k);
    s.solids.iter_mut().flat_map(|s| s.quads.iter_mut()).flatten().for_each(|p| *p *= k);
    s
}


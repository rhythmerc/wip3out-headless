//! The game's own sound: the player's game library (`WO3_ENGINE_LIB`) and their disc's cue sheet (`WO3_DISC`; skipped
//! without them). The ship idles, then thrusts across an 8 m room into its wall, turns and goes again, its sound taken
//! through the engine's stream as the runtime takes it (four blocks a frame): the race's start sounds are kept out,
//! and from then it is heard, without a gap, as loud after its wall hits as before them, and not clipped.
//! `WO3_SOUND_WAV=path` writes it (mono, 44.1 kHz) with the ship's speed each frame beside it (`path.csv`: frame,
//! speed in real m/s, thrust held) - a recording of the player's game: never kept in a repo.
use engine_core::glam::Vec3;
use engine_core::{PadHeld, PlacePlayer, Stage, TouchState};
use headless_core::library::test_paths;
use std::io::Write;
use wip3out_engine::stage::DEFAULT_SCALE as S;

fn wall(a: Vec3, b: Vec3, height: f32) -> [Vec3; 4] {
    let up = Vec3::Y * height;
    let q = [a, b, b + up, a + up];
    let n = (q[1] - q[0]).cross(q[2] - q[0]);
    // (its front to the room's middle)
    if n.dot(-a) > 0.0 { q } else { [q[3], q[2], q[1], q[0]] }
}

fn write_wav(path: &std::path::Path, samples: &[f32], rate: u32) -> std::io::Result<()> {
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    let data = (samples.len() * 2) as u32;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?; // PCM
    f.write_all(&1u16.to_le_bytes())?; // mono
    f.write_all(&rate.to_le_bytes())?;
    f.write_all(&(rate * 2).to_le_bytes())?;
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data.to_le_bytes())?;
    for s in samples {
        f.write_all(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())?;
    }
    f.flush()
}

#[test]
fn the_ship_is_heard() {
    let Some([library, cue]) = test_paths(["WO3_ENGINE_LIB", "WO3_DISC"]) else { return };
    let data = std::env::temp_dir().join(format!("wo3-sound-{}", std::process::id()));
    let mut engine = wip3out_engine::engine::factory(library, cue, data.clone())().expect("the game");
    let h = 4.0 * S;
    let c = |x: f32, z: f32| Vec3::new(x * h, 0.0, z * h);
    let corners = [c(-1.0, -1.0), c(1.0, -1.0), c(1.0, 1.0), c(-1.0, 1.0)];
    let mut quads = vec![[corners[0], corners[3], corners[2], corners[1]]];
    for i in 0..4 {
        quads.push(wall(corners[i], corners[(i + 1) % 4], 0.3 * S));
    }
    engine.set_stage(&Stage { revision: 1, room_quads: quads, ..Default::default() });
    while !engine.ready_to_place() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let mut stream = (engine.stream().expect("the game's sound"))().expect("its stream");
    let rate = stream.rate();
    let blocks = (0.04 / stream.block().as_secs_f64()).round() as usize;
    engine.place_player(PlacePlayer { point: Vec3::ZERO, forward: Vec3::NEG_Z });

    let idle = TouchState::default();
    let thrust = TouchState { right_trigger: 1.0, ..Default::default() };
    let turn = TouchState { right_trigger: 1.0, left_stick: [-1.0, 0.0], ..Default::default() };
    let mut pcm = Vec::new();
    let mut rows = Vec::new();
    let mut last: Option<Vec3> = None;
    for frame in 0..450 {
        let (input, held) = match frame {
            0..75 => (&idle, false),
            75..250 => (&thrust, true),
            250..290 => (&turn, true),
            290..400 => (&thrust, true),
            _ => (&idle, false),
        };
        engine.set_input(input, PadHeld::default());
        engine.step().expect("a frame");
        for _ in 0..blocks {
            pcm.extend(stream.render(false).expect("a block"));
        }
        let feet = engine.focus().expect("the ship").feet / S;
        let speed = last.map_or(0.0, |l| (feet - l).length() / 0.04);
        last = Some(feet);
        rows.push((frame, speed, held));
    }
    let rms = |s: &[f32]| (s.iter().map(|x| x * x).sum::<f32>() / s.len().max(1) as f32).sqrt();
    let at = |frame: usize| frame * pcm.len() / rows.len();
    // (the race's start sounds are kept out: its first 1.6 s silent)
    let start = rms(&pcm[at(5)..at(35)]);
    let (idling, flying) = (rms(&pcm[at(50)..at(75)]), rms(&pcm[at(100)..at(250)]));
    // (the second run, after the wall hits, against the first before them: the engine as loud)
    let (before, again) = (rms(&pcm[at(120)..at(180)]), rms(&pcm[at(310)..at(370)]));
    // (no gap: the longest silence after the start)
    let gap = pcm[at(50)..].split(|s| *s != 0.0).map(<[f32]>::len).max().unwrap_or(0);
    let peak = pcm.iter().fold(0f32, |m, s| m.max(s.abs()));
    let clipped = pcm.iter().filter(|s| s.abs() > 0.999).count();
    let top = rows.iter().map(|r| r.1).fold(0f32, f32::max);
    eprintln!(
        "sound: {:.1} s at {rate} Hz, rms at the start {start:.4}, idling {idling:.4}, flying {flying:.4}, before the hits {before:.4}, after {again:.4}, peak {peak:.3}, \
         {clipped} clipped, longest silence {gap} samples; top speed {top:.2} m/s",
        pcm.len() as f64 / f64::from(rate)
    );
    if let Some(path) = std::env::var_os("WO3_SOUND_WAV") {
        let path = std::path::PathBuf::from(path);
        write_wav(&path, &pcm, rate).expect("the wav");
        let mut csv = String::from("frame,speed,thrust\n");
        for (f, s, t) in &rows {
            csv += &format!("{f},{s:.4},{}\n", u8::from(*t));
        }
        std::fs::write(path.with_extension("csv"), csv).expect("the csv");
    }
    assert!(start < 0.001, "the race's start sounds kept out: rms {start}");
    assert!(idling > 0.005 && flying > 0.005, "the ship heard idling and flying: rms {idling}, {flying}");
    assert!(again > 0.8 * before, "the engine as loud after the wall hits: rms {again} against {before}");
    assert!(gap < 441, "no gap in its sound: {gap} samples silent");
    assert!(clipped < pcm.len() / 1000, "not clipped: {clipped} samples");
    let _ = std::fs::remove_dir_all(data);
}

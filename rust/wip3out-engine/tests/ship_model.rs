//! The ship's own model, read from the game: the player's game library (`WO3_ENGINE_LIB`) and their disc's cue sheet
//! (`WO3_DISC`; skipped without them). Once the race is loaded the engine's model is the game's body and air brakes,
//! textured from the game's video memory, not the stand-in box; a picture of it is drawn to `WO3_MODEL_PNG` when that
//! is set (a file of the player's game: never kept in a repo).
use engine_core::glam::{Mat3, Vec3};
use engine_core::{Model, ModelSource};
use headless_core::library::test_paths;

#[test]
fn the_ships_own_model() {
    let Some([library, cue]) = test_paths(["WO3_ENGINE_LIB", "WO3_DISC"]) else { return };
    let data = std::env::temp_dir().join(format!("wo3-model-{}", std::process::id()));
    let mut engine = wip3out_engine::engine::factory(library, cue, data.clone())().expect("the game");
    let models = engine.models();
    assert_eq!(models.len(), 1);
    let ModelSource::Meshes { meshes, textures, .. } = &models[0].source else { panic!("meshes") };
    let triangles: usize = meshes.iter().map(|m| m.indices.len() / 3).sum();
    let (lo, hi) = meshes.iter().flat_map(|m| m.positions.iter()).fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), &p| {
        (lo.min(Vec3::from_array(p)), hi.max(Vec3::from_array(p)))
    });
    eprintln!("{} meshes, {} textures, {triangles} triangles, from {lo} to {hi} (game units)", meshes.len(), textures.len());
    assert!(textures.len() >= 1 && triangles > 100, "the game's own model, textured, not the box");
    assert!(hi.z - lo.z > 600.0 && hi.z - lo.z < 1500.0, "a ship's length along its nose: {}", hi.z - lo.z);
    assert!(hi.z - lo.z > hi.x - lo.x, "longer than it is wide");
    if let Some(path) = std::env::var_os("WO3_MODEL_PNG") {
        render(&models[0], std::path::Path::new(&path));
        eprintln!("drawn to {}", std::path::Path::new(&path).display());
    }
    let _ = std::fs::remove_dir_all(data);
}

/// A picture of a model: seen from ahead and to its left, a little above, textured, lit from above.
fn render(model: &Model, path: &std::path::Path) {
    // (four views side by side - three-quarters from ahead and above, from above, from its left, from behind - the
    // viewer on +z of the view, looking down -z)
    let views = [
        Mat3::from_rotation_x(0.5) * Mat3::from_rotation_y(-0.7),
        Mat3::from_rotation_x(std::f32::consts::FRAC_PI_2),
        Mat3::from_rotation_y(-std::f32::consts::FRAC_PI_2),
        Mat3::from_rotation_y(std::f32::consts::PI),
    ];
    let (w, h) = (320usize, 240usize);
    let pictures: Vec<Vec<[u8; 3]>> = views.iter().map(|&v| picture(model, v, w, h)).collect();
    let mut data = Vec::with_capacity(4 * w * h * 3);
    for y in 0..h {
        for p in &pictures {
            data.extend(p[y * w..(y + 1) * w].iter().flat_map(|c| c.iter().copied()));
        }
    }
    let file = std::fs::File::create(path).expect("the picture's file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), (4 * w) as u32, h as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(&data).unwrap();
}

/// One view of a model, `w` by `h`.
fn picture(model: &Model, view: Mat3, w: usize, h: usize) -> Vec<[u8; 3]> {
    let ModelSource::Meshes { meshes, textures, .. } = &model.source else { return vec![[0; 3]; w * h] };
    let all: Vec<Vec3> = meshes.iter().flat_map(|m| m.positions.iter().map(|&p| view * Vec3::from_array(p))).collect();
    let (lo, hi) = all.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), &p| (lo.min(p), hi.max(p)));
    let scale = 0.9 * (w as f32 / (hi.x - lo.x)).min(h as f32 / (hi.y - lo.y));
    let centre = (lo + hi) * 0.5;
    let to_screen = |p: Vec3| {
        let q = view * p - centre;
        Vec3::new(w as f32 / 2.0 + q.x * scale, h as f32 / 2.0 - q.y * scale, q.z)
    };
    let light = Vec3::new(0.3, 1.0, 0.4).normalize();
    let mut colour = vec![[40u8, 44, 52]; w * h];
    let mut depth = vec![f32::MIN; w * h];
    for mesh in meshes {
        let tex = mesh.material.texture.map(|t| &textures[t as usize]);
        for tri in mesh.indices.chunks(3) {
            let v = [usize::from(tri[0]), usize::from(tri[1]), usize::from(tri[2])];
            let p = v.map(|i| to_screen(Vec3::from_array(mesh.positions[i])));
            let n = (view * Vec3::from_array(mesh.normals[v[0]])).normalize_or_zero();
            let shade = 0.45 + 0.55 * n.dot(light).abs();
            let (minx, maxx) = (p.iter().map(|q| q.x).fold(f32::MAX, f32::min).max(0.0) as usize, p.iter().map(|q| q.x).fold(f32::MIN, f32::max).min(w as f32 - 1.0) as usize);
            let (miny, maxy) = (p.iter().map(|q| q.y).fold(f32::MAX, f32::min).max(0.0) as usize, p.iter().map(|q| q.y).fold(f32::MIN, f32::max).min(h as f32 - 1.0) as usize);
            let area = (p[1].x - p[0].x) * (p[2].y - p[0].y) - (p[2].x - p[0].x) * (p[1].y - p[0].y);
            if area.abs() < 1e-6 {
                continue;
            }
            for y in miny..=maxy {
                for x in minx..=maxx {
                    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = ((p[1].x - fx) * (p[2].y - fy) - (p[2].x - fx) * (p[1].y - fy)) / area;
                    let w1 = ((p[2].x - fx) * (p[0].y - fy) - (p[0].x - fx) * (p[2].y - fy)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let z = w0 * p[0].z + w1 * p[1].z + w2 * p[2].z;
                    let at = y * w + x;
                    if z <= depth[at] {
                        continue;
                    }
                    let vc = [0, 1, 2].map(|c| w0 * mesh.colors[v[0]][c] + w1 * mesh.colors[v[1]][c] + w2 * mesh.colors[v[2]][c]);
                    let mut rgb = [vc[0], vc[1], vc[2]];
                    if let Some(t) = tex {
                        let u = w0 * mesh.uvs[v[0]][0] + w1 * mesh.uvs[v[1]][0] + w2 * mesh.uvs[v[2]][0];
                        let vv = w0 * mesh.uvs[v[0]][1] + w1 * mesh.uvs[v[1]][1] + w2 * mesh.uvs[v[2]][1];
                        let (tx, ty) = (((u * t.width as f32) as usize).min(t.width as usize - 1), ((vv * t.height as f32) as usize).min(t.height as usize - 1));
                        let px = &t.pixels[4 * (ty * t.width as usize + tx)..][..4];
                        if px[3] < 128 {
                            continue;
                        }
                        rgb = [0, 1, 2].map(|c| rgb[c] * f32::from(px[c]) / 255.0);
                    }
                    depth[at] = z;
                    colour[at] = rgb.map(|c| ((c * shade).clamp(0.0, 1.0) * 255.0) as u8);
                }
            }
        }
    }
    colour
}

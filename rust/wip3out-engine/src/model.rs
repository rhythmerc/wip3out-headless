//! The local ship's own model, read from the game as it races: its body and air brakes (the game's objects, from the
//! player's disc, decompressed and loaded by the game itself) and their textures from the PlayStation's video memory.
//! Nothing of it is kept: it is read each time the engine is made.
//!
//! An object (`wo3_engine.h`, `wo3_ship_model`): its name, vertex and primitive counts, and pointers to its vertices
//! (in pairs: x, y, z, then z, x, y halves), its primitives and its matrix (3x3 x4096, then its position). A primitive: its type (byte), flags
//! (byte), then by type - 1: a flat triangle (pad, three vertex indices, pad, RGB and a GPU code: 12 bytes), 2: a
//! textured triangle (two halves, the palette (CLUT) and texture page as the GPU takes them, three UV pairs and a pad,
//! three vertex indices and a pad, RGB and code: 24 bytes), 3: a flat quad (12 bytes), 4: a textured quad (24 bytes,
//! four UV pairs and four indices). Its axes are the ship's: x to its left, y down, z along its nose.
use crate::native::Game;
use engine_core::glam::{Mat3, Vec3};
use engine_core::hud::Rgba8;
use engine_core::model::{AlphaMode, Lighting, MaterialDesc, MeshData};
use engine_core::{Model, ModelSource};
use std::collections::BTreeMap;

/// One object, as read: its vertices (game units, its own axes), its primitives, and its matrix and position.
struct Object {
    vertices: Vec<Vec3>,
    primitives: Vec<Primitive>,
    turn: Mat3,
    position: Vec3,
}

#[derive(Clone, Debug)]
struct Primitive {
    indices: Vec<usize>,
    /// (texture page, palette) and a UV a corner, when textured.
    texture: Option<(u16, u16)>,
    uvs: Vec<[u8; 2]>,
    rgb: [u8; 3],
}

fn read(game: &Game, address: u32, len: usize) -> Result<Vec<u8>, String> {
    let mut out = vec![0u8; len];
    if game.read(address, &mut out) { Ok(out) } else { Err(format!("0x{address:08X}: not readable")) }
}

fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn i16_at(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}

fn object(game: &Game, address: u32) -> Result<Object, String> {
    let header = read(game, address, 0x28)?;
    let primitive_count = i16_at(&header, 0x12).max(0) as usize;
    let (vertices_at, primitives_at, matrix_at) = (u32_at(&header, 0x18), u32_at(&header, 0x1C), u32_at(&header, 0x20));
    // (the primitives first: they say how many vertices there are)
    let mut primitives = Vec::with_capacity(primitive_count);
    let mut at = primitives_at;
    for _ in 0..primitive_count {
        let head = read(game, at, 2)?;
        let size = match head[0] {
            1 | 3 => 12,
            2 | 4 => 24,
            t => return Err(format!("0x{at:08X}: a primitive of an unknown type {t}")),
        };
        let b = read(game, at, size)?;
        let corners = if b[0] == 1 || b[0] == 2 { 3 } else { 4 };
        let primitive = if size == 12 {
            Primitive { indices: b[4..4 + corners].iter().map(|&i| usize::from(i)).collect(), texture: None, uvs: Vec::new(), rgb: [b[8], b[9], b[10]] }
        } else {
            Primitive {
                indices: b[16..16 + corners].iter().map(|&i| usize::from(i)).collect(),
                texture: Some((u16_at(&b, 6), u16_at(&b, 4))),
                uvs: (0..corners).map(|c| [b[8 + 2 * c], b[9 + 2 * c]]).collect(),
                rgb: [b[20], b[21], b[22]],
            }
        };
        primitives.push(primitive);
        at += size as u32;
    }
    let vertex_count = primitives.iter().flat_map(|p| p.indices.iter().copied()).max().map_or(0, |m| m + 1);
    // (vertices in pairs, six halves a pair, as the game's renderer takes them two at a time: the first x, y, z; the
    // second z, then x, y)
    let raw = read(game, vertices_at, vertex_count.div_ceil(2) * 12)?;
    let s = |k: usize| i16_at(&raw, 2 * k) as f32;
    let vertices = (0..vertex_count)
        .map(|i| {
            let b = 6 * (i / 2);
            if i % 2 == 0 { Vec3::new(s(b), s(b + 1), s(b + 2)) } else { Vec3::new(s(b + 4), s(b + 5), s(b + 3)) }
        })
        .collect();
    let m = read(game, matrix_at, 0x20)?;
    let e = |i: usize| i16_at(&m, 2 * i) as f32 / 4096.0;
    // (rows as the game's GTE takes them: world = turn * vertex + position)
    let turn = Mat3::from_cols(Vec3::new(e(0), e(3), e(6)), Vec3::new(e(1), e(4), e(7)), Vec3::new(e(2), e(5), e(8)));
    let position = Vec3::new(u32_at(&m, 0x14) as i32 as f32, u32_at(&m, 0x18) as i32 as f32, u32_at(&m, 0x1C) as i32 as f32);
    Ok(Object { vertices, primitives, turn, position })
}

/// A texture page with its palette, from video memory, as RGBA (256 by 256 texels; colour 0 see-through, as the
/// PlayStation draws it).
fn texture(game: &Game, page: u16, palette: u16) -> Result<Rgba8, String> {
    let (px, py) = (u32::from(page & 0xF) * 64, u32::from((page >> 4) & 1) * 256);
    let depth = (page >> 7) & 3;
    let (cx, cy) = (u32::from(palette & 0x3F) * 16, u32::from(palette >> 6) & 511);
    let colours = if depth == 0 { 16 } else { 256 };
    let clut = if depth < 2 { game.read_vram(cx, cy, colours, 1).ok_or("the palette")? } else { Vec::new() };
    let words = match depth {
        0 => 64,
        1 => 128,
        _ => 256,
    };
    let rows = 256.min(512 - py);
    let width = words.min(1024 - px);
    let page_words = game.read_vram(px, py, width, rows).ok_or("the texture page")?;
    let mut pixels = vec![0u8; 256 * 256 * 4];
    for v in 0..rows as usize {
        for u in 0..256usize {
            let colour = match depth {
                0 => {
                    let w = page_words.get(v * width as usize + u / 4).copied().unwrap_or(0);
                    clut[usize::from((w >> ((u % 4) * 4)) & 0xF)]
                }
                1 => {
                    let w = page_words.get(v * width as usize + u / 2).copied().unwrap_or(0);
                    clut[usize::from((w >> ((u % 2) * 8)) & 0xFF)]
                }
                _ => page_words.get(v * width as usize + u).copied().unwrap_or(0),
            };
            let at = 4 * (v * 256 + u);
            let c5 = |s: u16| (((colour >> s) & 31) as u32 * 255 / 31) as u8;
            pixels[at..at + 4].copy_from_slice(&[c5(0), c5(5), c5(10), if colour == 0 { 0 } else { 255 }]);
        }
    }
    Ok(Rgba8 { width: 256, height: 256, pixels })
}

/// The local ship's model from the race: its body and air brakes in the body's space, as the model's (its forward +z,
/// its up +y, game units: the body's axes turned half round its nose), a mesh for each texture and one for its flat
/// faces, one joint.
pub fn ship(game: &Game) -> Result<Model, String> {
    let [body_at, left_at, right_at] = game.ship_model().ok_or("the race's ship models aren't there")?;
    let body = object(game, body_at)?;
    let mut parts = vec![(body.vertices.clone(), body.primitives.clone())];
    // (the air brakes as they sit now, carried into the body's space)
    let to_body = body.turn.transpose();
    for at in [left_at, right_at] {
        if let Ok(brake) = object(game, at) {
            let vertices = brake.vertices.iter().map(|&v| to_body * (brake.turn * v + brake.position - body.position)).collect();
            parts.push((vertices, brake.primitives));
        }
    }
    let to_model = |v: Vec3| Vec3::new(-v.x, -v.y, v.z);

    let mut by_texture: BTreeMap<Option<(u16, u16)>, MeshData> = BTreeMap::new();
    for (vertices, primitives) in &parts {
        for p in primitives {
            if p.indices.iter().any(|&i| i >= vertices.len()) {
                continue;
            }
            let mesh = by_texture.entry(p.texture).or_default();
            // (a PlayStation quad's corners go 0, 1, 2, 3 as two triangles 0-1-2 and 1-3-2)
            let triangles: &[[usize; 3]] = if p.indices.len() == 3 { &[[0, 1, 2]] } else { &[[0, 1, 2], [1, 3, 2]] };
            for t in triangles {
                let corners = t.map(|c| to_model(vertices[p.indices[c]]));
                let normal = (corners[1] - corners[0]).cross(corners[2] - corners[0]).normalize_or_zero();
                for (k, &c) in t.iter().enumerate() {
                    mesh.indices.push(mesh.positions.len() as u16);
                    mesh.positions.push(corners[k].to_array());
                    mesh.normals.push(normal.to_array());
                    let uv = p.uvs.get(c).map_or([0.0, 0.0], |uv| [(f32::from(uv[0]) + 0.5) / 256.0, (f32::from(uv[1]) + 0.5) / 256.0]);
                    mesh.uvs.push(uv);
                    // (the GPU's colour, 128 a full one, modulating the texture)
                    let c = |x: u8| (f32::from(x) / 128.0).min(1.0);
                    mesh.colors.push([c(p.rgb[0]), c(p.rgb[1]), c(p.rgb[2]), 1.0]);
                    mesh.joints.push([0; 4]);
                    mesh.weights.push([1.0, 0.0, 0.0, 0.0]);
                }
            }
        }
    }
    let mut meshes = Vec::new();
    let mut textures = Vec::new();
    for (key, mut mesh) in by_texture {
        mesh.material = MaterialDesc { two_sided: true, vertex_color: true, lighting: Lighting::default(), ..MaterialDesc::default() };
        if let Some((page, palette)) = key {
            textures.push(texture(game, page, palette)?);
            mesh.material.texture = Some(textures.len() as u32 - 1);
            mesh.material.alpha = AlphaMode::Mask(0.5);
        }
        meshes.push(mesh);
    }
    if meshes.is_empty() {
        return Err("the ship's model has no faces".into());
    }
    Ok(Model { source: ModelSource::Meshes { meshes, textures, joint_count: 1 }, parents: vec![None], groups: Vec::new() })
}

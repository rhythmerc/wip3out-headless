//! The runtime's world in the game's space, and the ship in the world. The game's axes are the PlayStation's (x right,
//! y down, z forward): the world turned half round its x ([`headless_core::space::Y_DOWN`]), so the game's left is the
//! player's left. The world's unit is the metre as the game sees it (the runtime shows the room bigger or smaller: its
//! world scale), [`UNITS`] game units each: the ship (its body 829 units long) is some 5.2 m long, an anti-gravity
//! racer's size, and the circuits' track some 21 m wide. A toy-sized ship in the room is [`DEFAULT_SCALE`].
use crate::native::Ship;
use engine_core::Stage;
use engine_core::glam::{Mat4, Vec3, Vec4};
use headless_core::space::{GameSpace, Y_DOWN};

/// Game units a (game) metre.
pub const UNITS: f32 = 160.0;

/// The world scale (game metres a real one) that makes the ship a toy about 20 cm long over the room's
/// floor: the runtime's default for this engine ([`crate::INFO`]), as Mario Kart Wii's is 8.
pub const DEFAULT_SCALE: f32 = 26.0;

/// How high the ship hovers over the ground, game units (as the game holds it over its own track).
pub const HOVER: f32 = 240.0;

/// The world in the game's space at `units` game units a metre: its origin at the game's.
pub fn space(units: f32) -> GameSpace {
    GameSpace::new(units, Y_DOWN)
}

/// The stage's triangles in the game's space, as the library takes them, each wound as it was: a wall's front (its
/// corners counter-clockwise) faces where the ship flies - the library's walls are one-sided.
pub fn triangles(stage: &Stage, units: f32) -> Vec<[f32; 9]> {
    space(units)
        .triangles(stage)
        .into_iter()
        .map(|[a, b, c]| [a.x, a.y, a.z, b.x, b.y, b.z, c.x, c.y, c.z])
        .collect()
}

/// Where the ship goes for the player put down at `point` facing `forward` (world): over the point by its hover, and
/// its heading (4096 a turn) - the game's ship faces `(-sin, 0, cos)` of it.
pub fn placement(point: Vec3, forward: Vec3, units: f32) -> ([i32; 3], i16) {
    let s = space(units);
    let p = s.point_to_game(point) + Vec3::NEG_Y * HOVER;
    let f = s.direction_to_game(forward);
    let yaw = (-f.x).atan2(f.z) * 4096.0 / std::f32::consts::TAU;
    ([p.x.round() as i32, p.y.round() as i32, p.z.round() as i32], yaw.round() as i16)
}

/// The ship's axes in the game's space: along its nose, its up, across it (to its right).
pub fn axes(ship: &Ship) -> (Vec3, Vec3, Vec3) {
    let row = |i: usize| Vec3::new(ship.basis[3 * i] as f32, ship.basis[3 * i + 1] as f32, ship.basis[3 * i + 2] as f32) / 4096.0;
    let forward = row(0).normalize_or(Vec3::Z);
    // (its third row is its down, as y is)
    let up = (-row(2)).normalize_or(Vec3::NEG_Y);
    (forward, up, up.cross(forward).normalize_or(Vec3::X))
}

/// The ship's frame in the world: a model made in game units (its forward +z, its up +y) at the ship's centre.
pub fn frame(ship: &Ship, units: f32) -> Mat4 {
    let s = space(units);
    let (forward, up, right) = axes(ship);
    let k = 1.0 / units;
    let w = |d: Vec3| (s.direction_to_world(d) * k).extend(0.0);
    let p = Vec3::new(ship.position[0] as f32, ship.position[1] as f32, ship.position[2] as f32);
    Mat4::from_cols(w(right), w(up), w(forward), Vec4::from((s.point_to_world(p), 1.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ship_facing(forward: Vec3, position: [i32; 3]) -> Ship {
        let down = Vec3::Y;
        let b = |v: Vec3| [(v.x * 4096.0) as i32, (v.y * 4096.0) as i32, (v.z * 4096.0) as i32];
        let right = (-down).cross(forward);
        let mut basis = [0; 9];
        basis[..3].copy_from_slice(&b(forward));
        basis[3..6].copy_from_slice(&b(right));
        basis[6..].copy_from_slice(&b(down));
        Ship { position, basis, ..Default::default() }
    }

    #[test]
    fn the_ship_hovers_over_where_it_is_put() {
        let (p, yaw) = placement(Vec3::new(1.0, 0.0, -0.5), Vec3::X, UNITS);
        assert_eq!(p, [160, -240, 80]);
        // (facing +x in the world: +x in the game too, (-sin, cos) of -a quarter turn)
        assert_eq!(yaw, -1024);
        let (_, yaw) = placement(Vec3::ZERO, Vec3::NEG_Z, UNITS);
        assert_eq!(yaw, 0, "the world's -z is the game's +z");
    }

    #[test]
    fn the_ships_frame_in_the_world() {
        let ship = ship_facing(Vec3::X, [160, -240, 80]);
        let m = frame(&ship, UNITS);
        let origin = m.transform_point3(Vec3::ZERO);
        assert!((origin - Vec3::new(1.0, 1.5, -0.5)).length() < 1e-4, "{origin}");
        // (its model's +z along its nose: the world's +x; its +y up)
        assert!((m.transform_vector3(Vec3::Z).normalize() - Vec3::X).length() < 1e-3);
        assert!((m.transform_vector3(Vec3::Y).normalize() - Vec3::Y).length() < 1e-3);
        assert!(m.determinant() > 0.0, "not mirrored");
        // (half the units a metre: everything twice as far, the ship drawn twice as big)
        let m2 = frame(&ship, UNITS / 2.0);
        assert!((m2.transform_point3(Vec3::ZERO) - Vec3::new(2.0, 3.0, -1.0)).length() < 1e-4);
        assert!((m2.transform_vector3(Vec3::Z).length() - 2.0 * m.transform_vector3(Vec3::Z).length()).abs() < 1e-6);
    }

    #[test]
    fn a_floor_in_the_games_space() {
        let stage = Stage { revision: 1, room_quads: vec![[Vec3::ZERO, Vec3::Z, Vec3::new(1.0, 0.0, 1.0), Vec3::X]], ..Default::default() };
        let t = triangles(&stage, UNITS);
        assert_eq!(t.len(), 2);
        assert_eq!(t[0][3..6], [0.0, 0.0, -160.0]);
    }
}

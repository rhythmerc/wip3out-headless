//! Free play runs on: the player's game library (`WO3_ENGINE_LIB`) and their disc's cue sheet (`WO3_DISC`; skipped
//! without them). Ten minutes of the game's time (15 000 frames) flying round a walled room, thrust held, turning left
//! now and then: the race never ends (its checkpoint clock held full), the ship flies on every minute (never
//! destroyed by the walls), it is the race's only ship, the game's other ship slots never move, and it stays on the
//! floor inside the walls.
use headless_core::library::test_paths;
use wip3out_engine::native::{FLYING, Game, Pad, button};
use wip3out_engine::stage::{DEFAULT_SCALE, UNITS};

/// The game's (SCES-02845's) addresses: its ships in the race (half), its ship slots, and the checkpoint clock
/// (centiseconds left, half).
const SHIP_COUNT: u32 = 0x801973D0;
const SHIP_ARRAY: u32 = 0x801D7038;
const SHIP_SIZE: u32 = 0x16C;
const CHECKPOINT_CLOCK: u32 = 0x801934E8;

fn read_u32(game: &Game, address: u32) -> u32 {
    let mut b = [0u8; 4];
    assert!(game.read(address, &mut b));
    u32::from_le_bytes(b)
}

fn read_i16(game: &Game, address: u32) -> i16 {
    let mut b = [0u8; 2];
    assert!(game.read(address, &mut b));
    i16::from_le_bytes(b)
}

/// Every ship slot's position.
fn slots(game: &Game) -> Vec<[i32; 3]> {
    let ships = read_u32(game, SHIP_ARRAY);
    (0..12).map(|i| [0, 4, 8].map(|o| read_u32(game, ships + SHIP_SIZE * i + 0x58 + o) as i32)).collect()
}

#[test]
fn ten_minutes_of_free_play() {
    let Some([library, cue]) = test_paths(["WO3_ENGINE_LIB", "WO3_DISC"]) else { return };
    let data = std::env::temp_dir().join(format!("wo3-long-{}", std::process::id()));
    std::fs::create_dir_all(&data).unwrap();
    let game = Game::load(&library).expect("the game");
    let started = std::time::Instant::now();
    game.boot(&cue, &data).expect("boot");

    // (a room 8 m square at the default world scale, game units: a floor, walls all round facing in)
    let h = 4.0 * DEFAULT_SCALE * UNITS;
    let quad = |a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3]| [[a, b, c].concat(), [a, c, d].concat()];
    let mut tris: Vec<[f32; 9]> = Vec::new();
    let tri = |v: Vec<f32>| -> [f32; 9] { v.try_into().unwrap() };
    for t in quad([-h, 0.0, -h], [h, 0.0, -h], [h, 0.0, h], [-h, 0.0, h]) {
        tris.push(tri(t));
    }
    let up = -0.4 * DEFAULT_SCALE * UNITS;
    for (a, b) in [([-h, -h], [h, -h]), ([h, -h], [h, h]), ([h, h], [-h, h]), ([-h, h], [-h, -h])] {
        // (wound so its front faces the room's middle)
        for t in quad([a[0], 0.0, a[1]], [a[0], up, a[1]], [b[0], up, b[1]], [b[0], 0.0, b[1]]) {
            tris.push(tri(t));
        }
    }
    game.set_stage(&tris);

    // (the first step waits for the race)
    let thrust = Pad { buttons: button::CROSS, ..Pad::default() };
    game.step(&thrust).expect("the race");
    eprintln!("racing {:.1} s after boot", started.elapsed().as_secs_f32());
    game.place_ship([0, -240, 0], 0);

    let still = slots(&game);
    let (mut lowest_clock, mut flying, mut moved_slots) = (i16::MAX, 0, 0);
    let mut furthest = 0f32;
    let (mut minute_from, mut slowest_minute) = (None::<[i32; 3]>, f32::MAX);
    let mut path = 0f32;
    let mut last: Option<[i32; 3]> = None;
    let flying_start = std::time::Instant::now();
    for frame in 0..15_000u32 {
        // (steered left one second in three: round the room, not into a corner and held there)
        let steer = if (frame / 25) % 3 == 0 { button::LEFT } else { 0 };
        game.step(&Pad { buttons: button::CROSS | steer, ..Pad::default() }).expect("a frame");
        if frame % 25 != 0 {
            continue;
        }
        let ship = game.ship().expect("the local ship");
        assert_eq!(read_i16(&game, SHIP_COUNT), 1, "frame {frame}: a solo race");
        let clock = read_i16(&game, CHECKPOINT_CLOCK);
        lowest_clock = lowest_clock.min(clock);
        if ship.flags & FLYING != 0 {
            flying += 1;
        }
        if let Some(l) = last {
            path += ((0..3).map(|k| ((ship.position[k] - l[k]) as f32).powi(2)).sum::<f32>()).sqrt();
        }
        last = Some(ship.position);
        if frame % 1500 == 0 {
            if minute_from.is_some() {
                slowest_minute = slowest_minute.min(path);
            }
            minute_from = Some(ship.position);
            path = 0.0;
        }
        let x = ship.position[0] as f32;
        let z = ship.position[2] as f32;
        furthest = furthest.max(x.abs()).max(z.abs());
        let now = slots(&game);
        // (the local ship's slot is the one where it is)
        moved_slots += still.iter().zip(&now).filter(|(a, b)| a != b && **b != ship.position).count();
        if frame % 1500 == 0 {
            eprintln!("minute {}: ship at ({:.0}, {:.0}) m (real), clock {clock}, flags 0x{:x}", frame / 1500,
                      x / (DEFAULT_SCALE * UNITS), z / (DEFAULT_SCALE * UNITS), ship.flags);
        }
    }
    eprintln!(
        "ten minutes flown in {:.0} s: the clock at least {lowest_clock}, {flying} samples off the ground, furthest {:.2} m (real) of {:.2}",
        flying_start.elapsed().as_secs_f32(),
        furthest / (DEFAULT_SCALE * UNITS),
        h / (DEFAULT_SCALE * UNITS)
    );
    // (the walls' hard hits wear a ship's energy down, and at its last the game destroys it: the library holds the
    // local ship's full, so it flies on)
    let slowest = slowest_minute / (DEFAULT_SCALE * UNITS);
    eprintln!("the least flown in a minute: {slowest:.1} m (real)");
    assert!(slowest > 2.0, "it flies on, every minute: {slowest} m");
    assert!(lowest_clock > 4000, "the race's clock never runs down: {lowest_clock}");
    assert_eq!(moved_slots, 0, "the game's other ship slots never move");
    assert!(furthest <= h * 1.02, "inside the walls: {furthest} of {h}");
    assert!(flying < 30, "over the ground: {flying} samples flying");
    let _ = std::fs::remove_dir_all(data);
}

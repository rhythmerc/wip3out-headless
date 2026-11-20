//! The game library (`libwip3out_engine`: psxrecomp's translation of the player's disc, headless), loaded at run time
//! from where the player keeps it, and its C API (`wo3_engine.h`).
use headless_core::library::GameLibrary;
use std::ffi::{CString, c_char, c_int};
use std::path::Path;

/// Pad 1 (`Wo3Pad`): PlayStation buttons, set bits pressed ([`button`]); sticks 0..255 (0x80 centred), analog nonzero
/// for a DualShock in analog mode.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pad {
    pub buttons: u16,
    pub lx: u8,
    pub ly: u8,
    pub rx: u8,
    pub ry: u8,
    pub analog: u8,
}

impl Default for Pad {
    fn default() -> Self {
        Self { buttons: 0, lx: 0x80, ly: 0x80, rx: 0x80, ry: 0x80, analog: 0 }
    }
}

/// The pad's buttons.
pub mod button {
    pub const SELECT: u16 = 0x0001;
    pub const START: u16 = 0x0008;
    pub const UP: u16 = 0x0010;
    pub const RIGHT: u16 = 0x0020;
    pub const DOWN: u16 = 0x0040;
    pub const LEFT: u16 = 0x0080;
    pub const L2: u16 = 0x0100;
    pub const R2: u16 = 0x0200;
    pub const L1: u16 = 0x0400;
    pub const R1: u16 = 0x0800;
    pub const TRIANGLE: u16 = 0x1000;
    pub const CIRCLE: u16 = 0x2000;
    pub const CROSS: u16 = 0x4000;
    pub const SQUARE: u16 = 0x8000;
}

/// The local ship after the last frame (`Wo3Ship`), in the game's units and axes (x right, y down, z forward): its
/// position, velocity, angles (4096 a turn), its axes (x4096, three rows: along its nose, across it, and down), the
/// track section it is over and its flags (0x10: flying, off the track), and the frames stepped.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ship {
    pub position: [i32; 3],
    pub velocity: [i32; 3],
    pub angles: [i16; 3],
    pub basis: [i32; 9],
    pub section: i32,
    pub flags: u32,
    pub frames: u32,
}

/// The ship's flag: off the track, flying (the game's rescue comes for a ship that stays so).
pub const FLYING: u32 = 0x10;

type Boot = unsafe extern "C" fn(*const c_char, *const c_char) -> c_int;
type Racing = unsafe extern "C" fn() -> c_int;
type Step = unsafe extern "C" fn(*const Pad) -> c_int;
type GetShip = unsafe extern "C" fn(*mut Ship) -> c_int;
type Read = unsafe extern "C" fn(u32, *mut u8, u32) -> c_int;
type SetStage = unsafe extern "C" fn(*const f32, u32);
type PlaceShip = unsafe extern "C" fn(*const i32, i16);
type SetShipTuning = unsafe extern "C" fn(f32, f32);

/// The loaded game. One a process: the game keeps its state in the library's globals, and the library stays loaded
/// when this goes (its threads still run in it).
pub struct Game {
    boot: Boot,
    racing: Racing,
    step: Step,
    ship: GetShip,
    read: Read,
    set_stage: SetStage,
    place_ship: PlaceShip,
    /// (a library built before it: none - the ship as the game has it)
    set_ship_tuning: Option<SetShipTuning>,
    _library: GameLibrary,
}

impl Game {
    /// The library at `path`.
    pub fn load(path: &Path) -> Result<Self, String> {
        // SAFETY: the library is the game built for this API (wo3_engine.h); its initialisers set up the runtime only.
        let library = unsafe { GameLibrary::load(path) }?;
        // SAFETY: each function is the C function of that name in wo3_engine.h, with that signature.
        unsafe {
            Ok(Self {
                boot: library.function("wo3_boot")?,
                racing: library.function("wo3_racing")?,
                step: library.function("wo3_step")?,
                ship: library.function("wo3_ship")?,
                read: library.function("wo3_read")?,
                set_stage: library.function("wo3_set_stage")?,
                place_ship: library.function("wo3_place_ship")?,
                set_ship_tuning: library.optional("wo3_set_ship_tuning"),
                _library: library,
            })
        }
    }

    /// Starts the game on its own thread from the disc's cue sheet at `cue`, its saves under `data_dir`; it drives its
    /// menus to a race by itself.
    pub fn boot(&self, cue: &Path, data_dir: &Path) -> Result<(), String> {
        let c = |p: &Path| CString::new(p.to_string_lossy().as_bytes()).map_err(|e| e.to_string());
        let (cue, data) = (c(cue)?, c(data_dir)?);
        // SAFETY: two NUL-terminated paths, read during the call (the library copies them).
        match unsafe { (self.boot)(cue.as_ptr(), data.as_ptr()) } {
            0 => Ok(()),
            e => Err(format!("wo3_boot: {e}")),
        }
    }

    /// The race runs: steps are frames from here.
    pub fn racing(&self) -> bool {
        // SAFETY: no arguments.
        unsafe { (self.racing)() != 0 }
    }

    /// One frame of the game (1/25 s) with `pad`.
    pub fn step(&self, pad: &Pad) -> Result<(), String> {
        // SAFETY: the pad is read during the call.
        match unsafe { (self.step)(pad) } {
            0 => Ok(()),
            e => Err(format!("wo3_step: {e}")),
        }
    }

    /// The local ship after the last frame.
    pub fn ship(&self) -> Option<Ship> {
        let mut ship = Ship::default();
        // SAFETY: the API writes the struct.
        (unsafe { (self.ship)(&mut ship) } == 0).then_some(ship)
    }

    /// The game's memory at guest `address`, between steps: `out` filled, or false where it isn't all there.
    pub fn read(&self, address: u32, out: &mut [u8]) -> bool {
        // SAFETY: the API writes at most `out.len()` bytes.
        unsafe { (self.read)(address, out.as_mut_ptr(), out.len() as u32) == 0 }
    }

    /// The ground to fly over: triangles in the game's units and axes (none: the circuit's own track again).
    pub fn set_stage(&self, triangles: &[[f32; 9]]) {
        // SAFETY: the API copies the triangles, nine floats each.
        unsafe { (self.set_stage)(triangles.as_ptr().cast(), triangles.len() as u32) }
    }

    /// The local ship put at `position`, heading `yaw` (4096 a turn), at rest, at the next frame.
    pub fn place_ship(&self, position: [i32; 3], yaw: i16) {
        // SAFETY: three ints read during the call.
        unsafe { (self.place_ship)(position.as_ptr(), yaw) }
    }

    /// The local ship's top speed and acceleration, each a multiple of its own (1: as the game has them; the library
    /// clamps them to 0.1..4). Taken at the next frame. False when the library can't (built before it).
    pub fn set_ship_tuning(&self, speed: f32, acceleration: f32) -> bool {
        let Some(set) = self.set_ship_tuning else { return false };
        // SAFETY: two plain values; callable from any thread.
        unsafe { set(speed, acceleration) };
        true
    }
}

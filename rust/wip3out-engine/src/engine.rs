//! The game as the runtime drives it ([`engine_core::EngineCore`]): free flight over the stage. The game boots once
//! made, drives its own menus to a single race (on its own thread), and from the race's first frame is stepped a frame
//! (1/25 s) at a time. The stage's triangles are the ground and walls the ship flies over in place of the circuit
//! (the library rebuilds the track section under the ship from them every frame); putting the player down puts the
//! ship there.
//!
//! The game's sound, HUD and laps are not taken yet; weapons are out of scope. The race is a solo race and its
//! checkpoint clock is held full by the library, so free play runs on.
use crate::native::{Game, Pad, Ship, button};
use crate::stage::{self, HOVER, UNITS};
use engine_core::glam::Vec3;
use engine_core::{Applied, EngineCore, EngineFactory, Focus, Model, PadHeld, PlacePlayer, Pose, Stage, TouchState};
use headless_core::input;
use std::path::PathBuf;
use std::time::Duration;

/// Its one model, the ship.
pub const SHIP: engine_core::ModelId = 0;

/// Its settings ([`EngineCore::set_setting`]), as Mario Kart Wii's engine has them. Its size in the room is the
/// runtime's world scale, as every engine's is ([`crate::stage::DEFAULT_SCALE`]).
pub mod settings {
    /// The ship's top speed, a multiple of its own (1: as the game has it; 0.1..4).
    pub const SPEED: &str = "speed";
    /// The ship's acceleration, a multiple of its own (1: as the game has it; 0.1..4).
    pub const ACCELERATION: &str = "acceleration";
}

/// The ship's size drawn (game units): about as the game's own collision has it - its nose 512 units ahead of its
/// centre, its wing tips 256 either side.
const SHIP_HALF_WIDTH: f32 = 260.0;
const SHIP_HALF_LENGTH: f32 = 520.0;
const SHIP_HEIGHT: f32 = 140.0;

/// How long the race may take to load before the engine is made without the ship's own model.
const RACE_LOADS_WITHIN: Duration = Duration::from_secs(90);

/// The game made on the runtime's worker: the library at `library` (built from the player's disc), the disc's cue
/// sheet at `cue`, its saves under `data`. It boots at once and is made once its race is loaded (some 8 seconds), its
/// ship drawn with the game's own model ([`crate::model`]).
pub fn factory(library: PathBuf, cue: PathBuf, data: PathBuf) -> EngineFactory {
    Box::new(move || {
        let game = Game::load(&library)?;
        std::fs::create_dir_all(&data).map_err(|e| format!("{}: {e}", data.display()))?;
        // (beside the library, or beside the disc: a host may load a copy of the library from elsewhere, as Android
        // must from the app's own storage)
        let beside = |p: &std::path::Path| p.parent().unwrap_or(std::path::Path::new(".")).to_path_buf();
        stage_runtime_files(&[beside(&library), beside(&cue)], &data)?;
        game.boot(&cue, &data)?;
        // (the ship's own model is the race's: read once the race is loaded, some 8 seconds on - this is the worker)
        let start = std::time::Instant::now();
        while !game.racing() && start.elapsed() < RACE_LOADS_WITHIN {
            std::thread::sleep(Duration::from_millis(50));
        }
        let model = crate::model::ship(&game).map_err(|e| eprintln!("Wip3out: the ship's model: {e}; a box stands in")).ok();
        Ok(Box::new(Wip3out::new(game, model)) as Box<dyn EngineCore>)
    })
}

/// The runtime's own files beside the game library or the disc (its configuration and BIOS), copied into the data folder the first
/// time: the library takes them from there, and keeps its writable state with them - on Android the library's folder
/// may not be writable, the data folder is the app's.
const RUNTIME_FILES: &[&str] = &["game.toml", "game_options.toml", "bios/openbios.bin", "bios/OpenBIOS.LICENSE"];

fn stage_runtime_files(folders: &[std::path::PathBuf], data: &std::path::Path) -> Result<(), String> {
    for file in RUNTIME_FILES {
        let dst = data.join(file);
        let Some(src) = folders.iter().map(|f| f.join(file)).find(|p| p.exists()) else { continue };
        if dst.exists() {
            continue;
        }
        if let Some(dir) = dst.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::copy(&src, &dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    }
    Ok(())
}

/// The running game.
pub struct Wip3out {
    game: Game,
    /// The last stage given, and a placement to make once the race runs.
    stage: Option<Stage>,
    place: Option<PlacePlayer>,
    /// The ship's top speed and acceleration ([`settings::SPEED`], [`settings::ACCELERATION`]).
    tuning: (f32, f32),
    pad: Pad,
    ship: Option<Ship>,
    model: Option<Model>,
    /// The ship's own model's parts, to pose (none: the stand-in), and their joints after the last frame.
    parts: Option<crate::model::ShipParts>,
    joints: Vec<engine_core::glam::Mat4>,
    posed: bool,
    cut: bool,
    /// A frame running on the game's thread, started at the last step (its results read at the next).
    running: bool,
}

impl Wip3out {
    fn new(game: Game, model: Option<(Model, crate::model::ShipParts)>) -> Self {
        let (model, parts) = match model {
            Some((m, p)) => (Some(m), Some(p)),
            None => (None, None),
        };
        Self {
            game,
            stage: None,
            place: None,
            tuning: (1.0, 1.0),
            pad: Pad::default(),
            ship: None,
            model: Some(model.unwrap_or_else(stand_in)),
            parts,
            joints: Vec::new(),
            posed: false,
            cut: true,
            running: false,
        }
    }

    /// Game units a world metre.
    fn units(&self) -> f32 {
        UNITS
    }

    /// The ship after the frame just done (the game still).
    fn read_frame(&mut self) {
        if let Some(ship) = self.game.ship() {
            self.ship = Some(ship);
            self.posed = true;
            if let Some(joints) = self.parts.and_then(|p| p.pose(&self.game)) {
                self.joints = joints;
            }
        }
    }
}

/// A stick past this is a d-pad direction.
const STICK_DEAD: f32 = 0.35;

/// The Touch controllers as the PlayStation pad, the game's own controls: the right trigger (or A) thrusts (CROSS),
/// the left stick steers and pitches (the d-pad), the grips are the air brakes (L1, R1), B fires (SQUARE).
///
/// A first go, digital: the game takes a DualShock's sticks too (analog), not yet mapped.
pub fn pad(touch: &TouchState) -> Pad {
    let mut buttons = 0;
    if touch.right_trigger > 0.3 || touch.a {
        buttons |= button::CROSS;
    }
    if touch.b {
        buttons |= button::SQUARE;
    }
    if touch.left_grip > 0.5 {
        buttons |= button::L1;
    }
    if touch.right_grip > 0.5 {
        buttons |= button::R1;
    }
    let (left, right) = input::axis_pushed(touch.left_stick[0], STICK_DEAD);
    let (down, up) = input::axis_pushed(touch.left_stick[1], STICK_DEAD);
    for (pushed, b) in [(left, button::LEFT), (right, button::RIGHT), (down, button::DOWN), (up, button::UP)] {
        if pushed {
            buttons |= b;
        }
    }
    Pad {
        buttons,
        lx: input::axis_u8(touch.left_stick[0], false),
        ly: input::axis_u8(touch.left_stick[1], true),
        rx: input::axis_u8(touch.right_stick[0], false),
        ry: input::axis_u8(touch.right_stick[1], true),
        analog: 0,
    }
}

/// A stand-in for the ship: a flat box its size (game units, its forward +z, its centre at its hover height).
fn stand_in() -> Model {
    let lo = Vec3::new(-SHIP_HALF_WIDTH, -SHIP_HEIGHT * 0.5, -SHIP_HALF_LENGTH);
    let hi = Vec3::new(SHIP_HALF_WIDTH, SHIP_HEIGHT * 0.5, SHIP_HALF_LENGTH);
    // (the nose a lighter blue, so it reads which way the ship faces)
    headless_core::model::box_model(lo, hi, [0.15, 0.35, 0.9, 1.0], [0.6, 0.8, 1.0, 1.0])
}

impl EngineCore for Wip3out {
    /// The game's frame: it updates its ships every other PAL vblank.
    fn period(&self) -> Duration {
        Duration::from_millis(40)
    }

    fn set_setting(&mut self, key: &str, value: f32) {
        // (the runtime gives its settings every frame: taken when they change)
        match key {
            settings::SPEED | settings::ACCELERATION => {
                let tuning = if key == settings::SPEED { (value, self.tuning.1) } else { (self.tuning.0, value) };
                if tuning == self.tuning {
                    return;
                }
                self.tuning = tuning;
                if !self.game.set_ship_tuning(tuning.0, tuning.1) {
                    eprintln!("Wip3out: this game library can't tune the ship (built before wo3_set_ship_tuning)");
                }
            }
            _ => {}
        }
    }

    fn set_input(&mut self, touch: &TouchState, held: PadHeld) {
        self.pad = if held.held { Pad::default() } else { pad(touch) };
    }

    /// The stage as the ground and walls the ship flies over, from the next frame (before the race: as it starts).
    fn set_stage(&mut self, stage: &Stage) -> Applied {
        if stage.revision == 0 {
            return Applied::Deferred;
        }
        if self.stage.as_ref().is_none_or(|s| s.revision != stage.revision) {
            self.game.set_stage(&stage::triangles(stage, self.units()));
            self.stage = Some(stage.clone());
        }
        Applied::Done
    }

    /// The ship put there, hovering, at rest (before the race: once it runs).
    fn place_player(&mut self, place: PlacePlayer) {
        if self.game.racing() {
            // (a frame running ends first, as it was, unseen: the ship is put down at the next, which is drawn cut)
            if std::mem::take(&mut self.running) {
                let _ = self.game.step_wait();
            }
            let (position, yaw) = stage::placement(place.point, place.forward, self.units());
            self.game.place_ship(position, yaw);
            self.cut = true;
        } else {
            self.place = Some(place);
        }
    }

    /// Once the race runs (the menus are driven on the game's own thread): a ship to put down.
    fn ready_to_place(&self) -> bool {
        self.game.racing()
    }

    fn wants_steps(&self) -> bool {
        self.game.racing()
    }

    /// A frame, its game's part run on the game's thread while the runtime goes on: the frame started at the last
    /// step finished (some 8 ms of the 40 a frame on a desktop, 20-40 on a Quest, mostly done by now) and read, and the
    /// next started with the controllers as they are. The poses are a frame (1/25 s) behind the controllers. The first
    /// step (or one after a placement) runs a frame whole first.
    fn step(&mut self) -> Result<(), String> {
        if let Some(place) = self.place.take() {
            self.place_player(place);
        }
        if std::mem::take(&mut self.running) {
            self.game.step_wait()?;
        } else {
            self.game.step(&self.pad)?;
        }
        self.read_frame();
        if self.game.steps_apart() && std::env::var_os("WO3_STEP_WHOLE").is_none() {
            self.running = self.game.step_begin(&self.pad)?;
        }
        Ok(())
    }

    fn focus(&self) -> Option<Focus> {
        let ship = self.ship.as_ref()?;
        let units = self.units();
        let frame = stage::frame(ship, units);
        let up = frame.y_axis.truncate().normalize_or(Vec3::Y);
        Some(Focus {
            feet: frame.w_axis.truncate() - up * (HOVER / units),
            up,
            forward: frame.z_axis.truncate().normalize_or(Vec3::NEG_Z),
            height: (HOVER + SHIP_HEIGHT * 0.5) / units,
        })
    }

    fn models(&mut self) -> Vec<Model> {
        self.model.take().into_iter().collect()
    }

    fn poses(&mut self, out: &mut Vec<Pose>) {
        if !std::mem::take(&mut self.posed) {
            return;
        }
        let Some(ship) = self.ship.as_ref() else { return };
        out.push(Pose {
            model: SHIP,
            discontinuity: std::mem::take(&mut self.cut),
            root: stage::frame(ship, self.units()),
            joints: if self.parts.is_some() && !self.joints.is_empty() {
                self.joints.clone()
            } else {
                vec![engine_core::glam::Mat4::IDENTITY; if self.parts.is_some() { usize::from(crate::model::JOINTS) } else { 1 }]
            },
            ..Pose::default()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_touch_controllers_fly() {
        let p = pad(&TouchState { right_trigger: 1.0, left_stick: [-1.0, 0.1], right_grip: 1.0, ..Default::default() });
        assert_eq!(p.buttons, button::CROSS | button::LEFT | button::R1);
        assert_eq!((p.lx, p.ly), (1, 115));
        assert_eq!(pad(&TouchState::default()), Pad::default());
        let p = pad(&TouchState { left_stick: [0.0, 0.9], ..Default::default() });
        assert_eq!(p.buttons, button::UP);
    }

    #[test]
    fn the_stand_in_is_a_closed_box() {
        let model = stand_in();
        let engine_core::ModelSource::Meshes { meshes, .. } = &model.source else { panic!("meshes") };
        assert_eq!((meshes[0].positions.len(), meshes[0].indices.len()), (24, 36));
    }
}

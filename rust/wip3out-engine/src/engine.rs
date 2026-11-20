//! The game as the runtime drives it ([`engine_core::EngineCore`]): free flight over the stage. The game boots once
//! made, drives its own menus to a single race (on its own thread), and from the race's first frame is stepped a frame
//! (1/25 s) at a time. The stage's triangles are the ground and walls the ship flies over in place of the circuit
//! (the library rebuilds the track section under the ship from them every frame); putting the player down puts the
//! ship there.
//!
//! A first go: the ship is drawn as a box; the game's sound, HUD, weapons and laps are not taken; the race's own
//! checkpoint clock still runs out (its game over ends the flight).
use crate::native::{Game, Pad, Ship, button};
use crate::stage::{self, HOVER, UNITS};
use engine_core::glam::Vec3;
use engine_core::{Applied, EngineCore, EngineFactory, Focus, Model, PadHeld, PlacePlayer, Pose, Stage, TouchState};
use headless_core::input;
use std::path::PathBuf;
use std::time::Duration;

/// Its one model, the ship.
pub const SHIP: engine_core::ModelId = 0;

/// Its settings ([`EngineCore::set_setting`]), as Mario Kart Wii's engine has them.
pub mod settings {
    /// The ship's top speed, a multiple of its own (1: as the game has it; 0.1..4).
    pub const SPEED: &str = "speed";
    /// The ship's acceleration, a multiple of its own (1: as the game has it; 0.1..4).
    pub const ACCELERATION: &str = "acceleration";
    /// The ship's size against the room, a multiple of its own (1: about a quarter of a metre long; 0.25..4). Its
    /// speed and hover scale with it: the game's whole world is that much bigger in the room.
    pub const SCALE: &str = "scale";
}

/// The ship's size drawn (game units): about as the game's own collision has it - its nose 512 units ahead of its
/// centre, its wing tips 256 either side.
const SHIP_HALF_WIDTH: f32 = 260.0;
const SHIP_HALF_LENGTH: f32 = 520.0;
const SHIP_HEIGHT: f32 = 140.0;

/// The game made on the runtime's worker: the library at `library` (built from the player's disc), the disc's cue
/// sheet at `cue`, its saves under `data`. It boots at once (its menus take some 25 seconds, driven by the library).
pub fn factory(library: PathBuf, cue: PathBuf, data: PathBuf) -> EngineFactory {
    Box::new(move || {
        let game = Game::load(&library)?;
        std::fs::create_dir_all(&data).map_err(|e| format!("{}: {e}", data.display()))?;
        game.boot(&cue, &data)?;
        Ok(Box::new(Wip3out::new(game)) as Box<dyn EngineCore>)
    })
}

/// The running game.
pub struct Wip3out {
    game: Game,
    /// The last stage given, and a placement to make once the race runs.
    stage: Option<Stage>,
    place: Option<PlacePlayer>,
    /// The ship's size against the room ([`settings::SCALE`]), and its top speed and acceleration
    /// ([`settings::SPEED`], [`settings::ACCELERATION`]).
    scale: f32,
    tuning: (f32, f32),
    pad: Pad,
    ship: Option<Ship>,
    model: Option<Model>,
    posed: bool,
    cut: bool,
}

impl Wip3out {
    fn new(game: Game) -> Self {
        Self {
            game,
            stage: None,
            place: None,
            scale: 1.0,
            tuning: (1.0, 1.0),
            pad: Pad::default(),
            ship: None,
            model: Some(stand_in()),
            posed: false,
            cut: true,
        }
    }

    /// Game units a world metre, at the scale asked.
    fn units(&self) -> f32 {
        UNITS / self.scale
    }

    /// The ship's size changed: the stage laid again at it, and the ship put back where it is, facing the same way.
    fn rescale(&mut self, scale: f32) {
        let (feet, forward) = match self.focus() {
            Some(f) => (Some(f.feet), f.forward),
            None => (None, Vec3::NEG_Z),
        };
        self.scale = scale;
        if let Some(stage) = self.stage.as_ref() {
            self.game.set_stage(&stage::triangles(stage, self.units()));
        }
        if let Some(feet) = feet.filter(|_| self.game.racing()) {
            self.place_player(PlacePlayer { point: feet, forward });
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
            settings::SCALE => {
                let scale = if value.is_finite() { value.clamp(0.25, 4.0) } else { 1.0 };
                if scale != self.scale {
                    self.rescale(scale);
                }
            }
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

    fn step(&mut self) -> Result<(), String> {
        if let Some(place) = self.place.take() {
            self.place_player(place);
        }
        self.game.step(&self.pad)?;
        if let Some(ship) = self.game.ship() {
            self.ship = Some(ship);
            self.posed = true;
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
            joints: vec![engine_core::glam::Mat4::IDENTITY],
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

//! WipEout 3 Special Edition's side of the buildAR engine contract, over psxrecomp's static recompilation of the
//! player's own disc. No Bevy, no game data: the game itself is a library built from the player's disc
//! (`libwip3out_engine`, its C API `wo3_engine.h`), loaded from where they keep it.
//!
//! - [`native`]: the game library, loaded.
//! - [`stage`]: the runtime's world in the game's units and axes, and the ship's place in it.
//! - [`model`]: the local ship's own model and textures, read from the game as it races.
//! - [`sound`]: the game's own sound, as the runtime's stream.
//! - [`engine`]: the game as the runtime drives it ([`engine_core::EngineCore`]): free flight over the stage.
//!
//! The ship flies over the room, not a circuit: the game keeps a ship on its track through track sections (a centre
//! and four faces: a wall, two base faces, a wall), and the library rebuilds the section under the ship every frame
//! from the stage's triangles, so the game's own hover, gravity and wall collision run on the room (the library's
//! `buildar/wo3_room.cpp`).
pub mod engine;
pub mod model;
pub mod native;
pub mod sound;
pub mod stage;

/// What the runtime knows of this engine before one is made: x1.0 is a ship toy-sized, about 46 cm long over the
/// room's floor (26 / 2.3 game metres a real one - the player's pick on the headset, 2.3 times the first 20 cm; the
/// ship is some 5.2 game metres long).
pub const INFO: engine_core::EngineInfo = engine_core::EngineInfo { natural_scale: 26.0 / 2.3 };

/// The game files: the player's disc (its cue sheet and tracks) with the game library built from it, its runtime's
/// configuration and BIOS beside it.
pub const DATA: engine_core::GameFiles = engine_core::GameFiles {
    drop: "Wip3out",
    folder: "wip3out",
    marker: "libwip3out_engine.so",
    beside: &[],
    how: &[
        "On a PC, once: build buildAR's WipEout 3 engine from your own",
        "disc (wip3out-headless), then copy its folder (the disc's cue and",
        "bins with libwip3out_engine.so, game.toml and bios/) to",
        "Download/Wip3out and import it here.",
    ],
};

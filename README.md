# wip3out-headless

WipEout 3 Special Edition (PAL, SCES-02845) as a buildAR engine: the game's own code, translated from the player's
disc by [psxrecomp](https://github.com/mstan/psxrecomp) through the
[WipEout 3 SE recompilation project](https://github.com/mstan/Wipeout-3-Special-Edition-Recomp), run headless as a
library the runtime steps, its ship flying free over the player's room.

No game data or game-derived code is in this repo. The game library is built on the player's machine from their own
disc.

## Layout

- `rust/wip3out-engine`: the engine contract (`engine_core::EngineCore`) over the game library:
  - `native.rs` loads the library and calls its C API (`wo3_engine.h`);
  - `stage.rs` maps the runtime's world into the game's units and axes;
  - `engine.rs` is the engine.
- Shared host pieces (loading a game library, input, space, stand-in model) come from
  [`buildar-headless-core`](../buildar-headless-core), a sibling repo. Mario Kart Wii's engine uses the same crate.

## The game library

It is built from a local clone of the recomp project, on its `buildar` branch (and psxrecomp's `buildar` branch inside
it). The branch adds:

- `buildar/` in the game repo:
  - the C API (`wo3_engine.h`, `engine_api.cpp`): boot headless on a thread, drive the menus to a single race, then
    step in lockstep a game frame at a time (25 Hz: the game updates its ships every other PAL vblank) with the host's
    pad, and read the local ship;
  - the room collision (`wo3_room.cpp`, below).
- psxrecomp:
  - `PSX_BUILDAR_SHARED`: the runtime as a shared library, its `main` renamed;
  - two weak hooks, `psx_buildar_vblank` (lockstep) and `psx_buildar_pad`;
  - fixes for a launcher-less library.

Build it (Linux, in a toolbox with cmake, ninja and gcc):

```sh
cd Wipeout-3-Special-Edition-Recomp            # the buildar branch
git submodule update --init --recursive
./psxrecomp/tools/ci/build_emitters.sh
python3 psxrecomp/psxrecomp_cli.py generate --config game.toml --project-root . \
    --disc "disc/WipEout 3 - Special Edition (Europe) (En,Fr,De,Es,It).cue"
cmake -S . -B build-lib -G Ninja -DCMAKE_BUILD_TYPE=Release -DPSX_BUILDAR_SHARED=ON -DPSX_RECOMP_UI=OFF
cmake --build build-lib --target psx-runtime      # build-lib/libwip3out_engine.so, game.toml, bios/
```

`disc/` holds the Redump-style multi-track dump: the cue sheet and one bin per track. Track 01's MD5 must be
`b9d48f139544051893f45eed62780e42`. A CHD converts with `chdman extractcd`, then split the bin per track at the
cue's `INDEX 00` points. The library finds `game.toml` and `bios/openbios.bin` beside itself.

## Tests

```sh
WO3_ENGINE_LIB=/path/to/build-lib/libwip3out_engine.so \
WO3_DISC="/path/to/WipEout 3 - Special Edition (Europe) (En,Fr,De,Es,It).cue" \
cargo test --release
```

Without the two variables, `tests/free_flight.rs` skips itself. With them, it boots the game, flies the ship over a
6 m walled floor, and checks the hover and the walls.

## Settings

As Mario Kart Wii's engine has them (`engine::settings`):

- `speed`: the ship's top speed, a multiple of its own (0.1..4). The library scales its thrust maximum.
- `acceleration`: a multiple of its own (0.1..4). The library scales the speed the game's physics adds along the
  ship's nose each frame while it gains.
- `scale`: the ship's size against the room (0.25..4; 1 is about a quarter of a metre long). The game's whole world
  scales with it in the room: its speed and hover too.

## How the ship flies over a room

WipEout 3 keeps a ship on its circuit through track sections, as the first WipEout does (its leaked source and
phoboslab's rewrite describe the same scheme). Each section has:

- a centre and prev/next links;
- four faces: a wall, two base faces and a wall.

Each frame the ship takes the section nearest it (`0x8012E7CC`). It then:

- hovers over that section's base faces (the plane distance from `0x800ED8AC`, the forces in `0x800F04E8`);
- counts as flying, and in time gets rescued, when it isn't over them (`0x800EAA04`);
- bumps off the section's walls, tested at its nose and wing tips (`0x800ED48C` → `0x800ECB6C`).

The library hooks the ships loop (`0x800F2534`, through the recompiler's `mod_function_entry_funcs`). Before the game
looks, it rewrites the section under the local ship from the room's triangles:

- the base faces go on the floor under the ship;
- the walls go on the nearest room walls;
- the neighbours' centres are strung out along those walls, so the section stays the nearest.

The game's own hover, gravity and wall response then run on the room.

- **The corridor is at most 6000 units (1.5 m) across.** Wider sections break the game's fixed-point physics: the
  hover bounces and the walls leak. With no wall near, both sides are open, 3000 units either side of the ship. They
  move with it every frame, so it never reaches them.
- **Walls are one-sided.** A stage triangle's front (counter-clockwise) faces the room. A ship that gets through one
  in a frame is put back in front of it, its velocity into the wall reversed.

## Not yet

- The ship is drawn as a box. The game's model and textures sit in the disc's `.PBP` archives, not read yet.
- No sound, HUD, weapons or laps.
- The race's checkpoint clock still runs. Its game over ends the flight after about 50 seconds without checkpoints.
- The AI ships still race the circuit.
- Input is digital. The game also takes a DualShock's sticks.
- Android: the library compiles and links for arm64 (a fiber shim for Bionic's missing `ucontext`). It hasn't run on
  a device.

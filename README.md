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
  - `model.rs` reads the local ship's own model and textures from the game as it races: its body, and its air
    brakes as joints posed each frame from the game's own matrices (their attachment to the body). They come from
    the game's memory and video memory once the race has loaded them, the game itself having decompressed them from
    the disc's `.PBP` archives (whose compression isn't decoded here);
  - `engine.rs` is the engine.
- Shared host pieces (loading a game library, input, space, stand-in model) come from
  [`buildar-headless-core`](../buildar-headless-core), a sibling repo. Mario Kart Wii's engine uses the same crate.

## The game library

It is built from a local clone of the recomp project, on its `buildar` branch (and psxrecomp's `buildar` branch inside
it). The branch adds:

- `buildar/` in the game repo:
  - the C API (`wo3_engine.h`, `engine_api.cpp`): boot headless on a thread, into a solo race, then step in lockstep
    a game frame at a time (25 Hz: the game updates its ships every other PAL vblank) with the host's pad - whole, or
    started and waited for apart (`wo3_step_begin`, `wo3_step_wait`); read the local ship, its models and the video
    memory; put the ship down at rest, level, as the game sets a ship on the grid;
  - into a race quickly (`wo3_race.cpp`): the boot screens skipped (the logos, the title and its attract movie), the
    menus skipped (their code made to return; what they leave set written at the race, `WO3_PAD_MENUS` to pass them
    with the pad instead), the race's ship count made one, its checkpoint clock and the
    local ship's energy (which wall hits wear down, to its destruction) held full.
    The race starts some 6 s after boot, against 23 s with the menus driven by frame counts;
  - the room collision (`wo3_room.cpp`, below);
  - no picture: the game's drawing is off (the host draws the ship from its model), textures still loaded.
- psxrecomp:
  - `PSX_BUILDAR_SHARED`: the runtime as a shared library, its `main` renamed (on Android always so);
  - two weak hooks, `psx_buildar_vblank` (lockstep) and `psx_buildar_pad`;
  - mod hooks that can take a function's place (`psx_mod_register_function_replace_plugin`);
  - `gpu_set_skip_draw`;
  - Android arm64: an AArch64 fiber switch (Bionic has no `ucontext`), GLES linked, `-fsigned-char`;
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
cue's `INDEX 00` points. The engine copies `game.toml` and `bios/` from beside the library into its data folder the
first time, and the library takes them, and keeps its writable state, there.

For Android arm64 (builds and links, not yet run on a device):

```sh
cmake -S . -B build-android -G Ninja -DCMAKE_BUILD_TYPE=Release -DPSX_DEBUG_TOOLS=OFF \
    -DCMAKE_TOOLCHAIN_FILE=$NDK/build/cmake/android.toolchain.cmake -DANDROID_ABI=arm64-v8a -DANDROID_PLATFORM=android-29
cmake --build build-android --target psx-runtime  # build-android/libwip3out_engine.so
```

## Tests

```sh
WO3_ENGINE_LIB=/path/to/build-lib/libwip3out_engine.so \
WO3_DISC="/path/to/WipEout 3 - Special Edition (Europe) (En,Fr,De,Es,It).cue" \
cargo test --release
```

Without the two variables the game's tests skip themselves. With them:

- `free_flight`: a 6 m walled floor in both windings; the ship hovers within 3 cm and stays inside the walls, and
  steps go on past where the race would end;
- `long_flight`: ten minutes of the game's time round an 8 m room - no game over, one ship, the other slots still,
  flying on every minute (never destroyed by the walls: the library holds its energy), on the floor inside the walls;
- `ship_tuning`: speed and acceleration;
- `ship_model`: the game's own model, textured; `WO3_MODEL_PNG=path` draws it in four views (a picture of the
  player's game: never kept in a repo);
- `corner`: flown into a corner of a walled room and out again;
- `room_dump` (also `WO3_STAGE_DUMP`: a room as the app dumped it, its `dump_stage` dev flag - the player's scan,
  never kept in a repo): the ship put down where the player put it on the headset (the floor, the bed, a pillow on
  it) at the dump's scale and at 26; left alone it stays put and upright (off the pillow it may slide, onto the
  bed), flown it stays in the room, the stage given again as a new revision as it flies. It prints what a step costs,
  back to back and between the caller's own frames.

## Settings and scale

As Mario Kart Wii's engine has them (`engine::settings`):

- `speed`: the ship's top speed, a multiple of its own (0.1..4). The library scales its thrust maximum.
- `acceleration`: a multiple of its own (0.1..4). The library scales the speed the game's physics adds along the
  ship's nose each frame while it gains.

Its size in the room is the runtime's world scale, as for every engine. The engine works in game metres, 160 game
units each: the ship (its body 829 units long) is about 5.2 m long, an anti-gravity racer's size. `stage::DEFAULT_SCALE` (26 game metres to a
real one) makes it a toy about 20 cm long. The app's `Engine::default_scale` would take that value for
this engine, as it takes 8 for Mario Kart Wii; `INFO` declares it (`EngineInfo::natural_scale`).

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
- **Walls are one-sided.** A stage triangle's front (counter-clockwise) faces the room. A scanned mesh's walls come in
  both windings, and the copy whose back faces the ship is passed through. A ship that went through a wall's front in
  the last frame is put back in front of it, its velocity into the wall reversed.
- **A wall is one the ship meets.** Its nearest point is more than 120 units over the floor (lower - a scan's floor
  noise, cables - the ship hovers over it) and at most 600 over the ship (higher - a shelf's underside, a ceiling's
  light - it passes under). The facing wall is one straight across, by its horizontal distance.
- **The floor's edge is a wall.** Past it (the scan's end, an open door) the ship is put back where it last had floor
  under it. Left on, the section stays behind, the game takes the far-off ship for lost, and its rescue drags it
  across the room. Over the floor a rescue, if one starts, is ended as the game ends it.

## What it costs

`examples/bench.rs` (`bench <library> <cue> [frames]`, desktop or an Android shell) times the factory to the race,
then a frame stepped whole, back to back, flown round a walled room. A desktop, before and after the library's
headless savings:

| | before | after |
|---|---|---|
| factory to racing | 5.37 s | 2.61 s |
| a game frame (of 40 ms) | 6.66 ms | 2.65 ms |

psxrecomp is cycle-exact: every guest instruction pays its cycle accounting, so a frame costs what the guest's cycles
do. The library (`buildar/wo3_speed.cpp`, `engine_api.cpp`) cuts those cycles:

- **The vsync wait is done natively.** The game spins in the PsyQ VSync loop for the next vertical blank. The library
  advances the guest onto each next deliverable event instead, delivering its interrupt there.
- **The race's scene isn't drawn.** About half the frame was the game transforming the track, sky and ships on the
  guest's GTE for a picture nobody sees; the ships' flight doesn't read it (`WO3_DRAW=1` draws it).
- **No sound is rendered.** No host takes it yet (`WO3_SOUND=1` renders it).
- **The runtime's text capture is off**, and **the BIOS boot reads the disc at 32x** (the game itself at its own
  speed).

The race's loading (some 360 vertical blanks of the game's own decompression) and its intro (330) remain.

## Steps off the runtime's thread

A step waits for the frame started at the step before (on the game's own thread, run while the runtime drew its
frames), reads it and starts the next: between the runtime's own frames a step holds it up 0.01 ms (`room_dump`,
desktop). The poses are a frame (40 ms) behind the controllers. `WO3_STEP_WHOLE=1` steps whole.

## Not yet

- **Corners:** the corridor takes one wall and the one facing it.
- **Missing features:** no sound, HUD or laps; weapons are out of scope.
- **Restoring the circuit:** `wo3_set_stage` with no triangles stops the rewriting, but doesn't restore the sections
  already rewritten.
- **Controls:** input is digital (the game also takes a DualShock's sticks). The air brakes on L1/R1 are untested.
- **Android:** the engine library builds and links for arm64 (and the crate for aarch64-linux-android); it hasn't
  run on a device. Headless with drawing off it makes no GL calls and needs no SDL activity; its files are in the
  data folder the host gives; the game's thread has a 64 MB stack.

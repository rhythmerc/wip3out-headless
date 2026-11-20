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
    a game frame at a time (25 Hz: the game updates its ships every other PAL vblank) with the host's pad; read the
    local ship, its models and the video memory;
  - into a race quickly (`wo3_race.cpp`): the boot screens skipped (the logos, the title and its attract movie), the
    menus passed with the pad as each screen takes it, the race's ship count made one, its checkpoint clock held full.
    The race starts some 8 s after boot, against 23 s with the menus driven by frame counts;
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
  on the floor inside the walls (and how far it flew in its slowest minute, below);
- `ship_tuning`: speed and acceleration;
- `ship_model`: the game's own model, textured; `WO3_MODEL_PNG=path` draws it in four views (a picture of the
  player's game: never kept in a repo).

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

## Not yet

- **A hard crash into a wall stalls the ship:** it can end on its side against the wall (rolled about 55 degrees),
  its thrust cut, the game's crashed flag (0x8000 in +0xEC) set. On a circuit the game's rescue lifts it out; here,
  with the section rebuilt under it, nothing does. In `long_flight` this happens about three minutes in, and it sits
  there for the rest (the race itself runs on). Setting it upright, clearing the flag and moving it off the wall was
  tried and isn't enough; the game's crash and recovery logic needs reading.
- **Corners:** the corridor takes one wall and the one facing it.
- **The menus are passed, not skipped:** skipping them was tried as far as this: with the front end's own setup kept,
  the menus' code made to return and what they leave set written, the race is entered at vblank 767 but fails setting
  up its ships - it reads ship and pilot records the menus build (`wo3_race.cpp`). Going in directly needs those
  built as the menus build them.
- **Missing features:** no sound, HUD or laps; weapons are out of scope.
- **Restoring the circuit:** `wo3_set_stage` with no triangles stops the rewriting, but doesn't restore the sections
  already rewritten.
- **Controls:** input is digital (the game also takes a DualShock's sticks). The air brakes on L1/R1 are untested.
- **Android:** the engine library builds and links for arm64 (and the crate for aarch64-linux-android); it hasn't
  run on a device. Headless with drawing off it makes no GL calls and needs no SDL activity; its files are in the
  data folder the host gives; the game's thread has a 64 MB stack.

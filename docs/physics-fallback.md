# A physics and collision core of our own: the fallback

WipEout 3's ship flies over the player's room on the game's own physics today. The library (`buildar/wo3_room.cpp`
in the game repo) rebuilds the track section under the ship from the room every frame, and the game's hover, gravity
and wall response run on it. This document is the shape of the fallback: a general physics and collision core of
ours, engine-agnostic, that any buildAR engine could drive. It says when to switch to it, from what driving the
game's physics on real rooms showed.

## Where driving the game's physics stands

The game keeps a ship on its circuit through track sections. Each is a centre, prev and next links, and four faces:
a wall, two base faces and a wall. The ship hovers over the section's base plane on a spring, aligns to it over a
few frames, falls under gravity off it, and bumps off its walls (tested at its nose and wing tips). The library makes
the section a corridor under the ship: the floor under its centre, the nearest wall that meets it, and the wall
facing that one.

What the replay tests (`tests/ramp.rs`, `tests/room_dump.rs`, `tests/corner.rs`, `tests/free_flight.rs`) hold now, at
the living room's scale (11.3) and at 7x and 26x:

| Case | Result |
|---|---|
| A wedge up to a platform, off its end at speed | launches (6-8 frames in the air), lands upright, flies on |
| A curved kicker narrower than the ship | launches (7 frames), lands upright |
| Off a table top | falls, lands, flies on |
| A long 15-degree slope to a plateau | climbs, stays on top |
| A low step (1.3 cm real) | hovered over |
| A tall step (13 cm real) | stops the ship |
| A ramp taken 35 degrees off its line | stays upright, gets over |
| Slow up a ramp | climbs part way, rolls back, upright |
| Put down on the floor, furniture, a pillow | stays put |
| Corners, walls in both windings, noisy scans | contained |

It took these rules on top of the game's physics, each standing in for something a general core would do directly:

- **A wall is one the ship meets.** A steep face counts only if it reaches up to the ship's underside, its top more
  than 120 units over the ground on the ship's side of it, there. Without this, flying off a platform's edge read
  the platform's drop face as a wall gone through: the ship was thrown back and stood on its nose. That was the
  headset bug. A narrow ramp's sides read as walls beside the ship riding it.
- **The ground is hard under the hover.** The game's spring is tuned for its tracks: a short steep ramp at speed
  passed up through the ship. Under its hover height, climbing, the ship's velocity into the ground's plane is
  turned along it, and it is never let sink below 100 units.
- **A wall's knock never adds speed.** The game answers a wall met inside the wing tips with a kick, and in a
  cluttered scan the corridor's nearest wall changes frame to frame. That nearly doubled the ship's speed in a
  frame. The gain a frame is capped.
- **At rest on the ground the ship holds**: no thrust and slow, its slide on a gentle slope is damped. The game's
  tracks have no pillows.
- **The floor's edge is a wall**, and the game's rescue is ended over the room.
- **The stage is in a grid**, so a frame visits nearby triangles only: 0.65 ms a frame on a desktop for a 38,000
  triangle living room.

### What the game's model can't do

These are structural, not tuning:

1. **One ground plane at a time.** The section has one base plane, from the floor under the ship's centre. A ship
   longer than the feature it's on (5.2 game metres: half a metre real at 11.3x, longer than the room's ramps) gets
   no say from its nose or tail. It doesn't pitch onto a ramp until its centre is on it, and its nose clips the ramp
   meanwhile. A step is a sudden plane change.
2. **Two walls, as parallel lines.** The corridor holds the nearest wall and its facing one. In a corner or among
   clutter the chosen wall changes frame to frame, and the game kicks the ship whenever a wall comes inside its wing
   tips. The knock cap hides the worst of it, but a hit still throws the ship up to most of its length in a frame.
3. **Gravity is the game's down.** The tracks bank and climb, but they don't loop or run up walls. The course design
   wants gravity against surface normals (ride up a wall, under a table, a loop). A section whose floor is a wall
   would need the game's whole world turned per section, and its camera, sky and AI would all disagree.
4. **No ceilings, no overhangs.** A table's underside, the ship under furniture: the corridor has no top.
5. **Tuning is the game's.** Hover height, spring, grip, wall response are in the game's code and data, in game
   units. The rules above are patches around them, each a threshold that holds for the cases tested.
6. **Every engine alone.** Mario Kart Wii does the same job its own way (a KCL built from the stage). Each engine
   relearns ramps, steps and noisy scans.

### When to switch

Keep the game's physics while the course stays ground with ramps, steps and furniture, and the replay suite passes.
Switch to the core, for the engines whose courses need it, when any of these lands:

- **Gravity along surface normals**: wall rides, loops, under-table runs. That is item 3 above, and no patch reaches
  it.
- **A fourth patch for the same family of bug** (ground contact, walls). The rules above are three-and-a-half
  already. Past that, maintaining thresholds around a model built for something else costs more than owning the
  model.
- **Course pieces with properties**: friction, boost, bounce, sticky surfaces. The game's faces have flags for its
  own kinds (speed pads, weapons) only.
- **A second engine wanting the same ramps to behave the same.**

## The shape of the core

A Rust crate beside `buildar-headless-core` (say `buildar-physics`). It is pure, deterministic and
engine-agnostic. It knows the stage, bodies and contacts, not any game. It works in stage metres at the runtime's
world scale, so a course behaves the same at any player size.

### Collision world

Built once per stage revision (on a worker: a scan is tens of thousands of triangles):

- **Triangles from the stage**: room quads, scanned triangles, placed solids. Per triangle: its normal, its source
  (scan, scene, placed piece, course piece with properties), its piece id.
- **Both windings merged.** A scan sends each face twice. Faces are deduplicated by their vertex set and made
  two-sided, with the side the body came from deciding the response. Today's one-sided walls exist only because of
  the game.
- **Noise.** Merging coplanar, adjacent scanned faces into patches gives fewer, larger faces, and stable normals for
  the ground. Features under a step height become "ground roughness", not walls.
- **Edges.** Shared edges carry an adjacency and a convexity flag, so a sweep along a floor doesn't snag on the seams
  between triangles (the internal-edge problem). The crease between a ramp and the floor is concave; a platform's lip
  is convex.
- **A bounding volume hierarchy** (or the uniform grid the library uses today, which held up at 38,000 triangles).

Queries:

- `ray(origin, dir, max) -> hit` (point, normal, distance, face, piece)
- `sweep(shape, from, to) -> first hit` for a sphere, a capsule or a box: continuous collision, so a fast body
  never tunnels through a thin ramp or wall.
- `closest(point, radius) -> contacts`
- `ground(point, up, reach) -> hit`: the probe the hover uses, along any up, not only world up.
- `overlap(shape) -> faces`

### Bodies: a hover vehicle

A rigid body (position, orientation, linear and angular velocity, mass, inertia) and:

- **A hull**: a capsule or box the ship's size, swept against the world each step.
- **Hover probes**: four or five rays from points under the hull (nose, tail, wing tips, centre) along the body's
  down. Each is a spring-damper: rest length (the hover height), stiffness, damping, a maximum force. A ship over a
  kicker feels it at the nose before its centre gets there; that is item 1 above.
- **Local gravity**: while the probes hold ground, gravity is the weighted ground normal's reverse, smoothed over a
  few steps. Airborne, it eases back to world down at a tunable rate, or keeps the last surface's for a "sticky"
  piece. This is the course's "gravity against surface normals", and what makes wall rides and loops possible.
- **States**: grounded (any probe in reach), airborne, landing (the step contact returns after being airborne),
  crashed (an impact over a threshold, for the game's crash effects).
- **Orientation**: up is aligned to the local gravity's reverse at an align rate. Yaw comes from steering. Pitch and
  roll are free in the air, with angular damping and optional auto-level.
- **Drive**: thrust along the nose, quadratic and linear drag, lateral grip (the sideways slip a hover ship has),
  airbrakes as one-sided drag and yaw. The engine's settings (speed, acceleration) scale thrust and top speed, as
  they do the game's today.

Stepping is fixed: a few substeps per game frame (say 4 at 25 Hz), semi-implicit Euler, the hull swept per
substep.

### Contacts and responses

- **Wall**: the velocity is projected onto the wall's plane, its normal part reflected by a restitution (small), the
  tangential part scaled by a wall friction. Never a kick: the response is a function of the impact, not of where
  the wall was last frame.
- **Glancing vs head-on**: the impact's normal speed over a threshold is a crash event, with speed loss; under it, a
  scrape event (sparks, sound).
- **Landing**: the probes absorb it. An impact over a threshold is a hard-landing event; landing more than a
  tolerance off the ground's normal is a crash.
- **Steps**: a ledge under the step height (taken from the probes' reach) is climbed by the suspension; over it, it
  is a wall.
- **Ceilings**: a hit on a face over the hull pushes the body down along the face, and caps the hover's lift there.

### Tunables

Everything a ship's feel is made of, in one table per vehicle (and per piece, for surfaces). Lengths are in the
vehicle's own body lengths where that keeps it scale-free:

| Group | Parameters |
|---|---|
| Hover | height, stiffness, damping, max force, probe layout, step height |
| Gravity | strength, align rate, air ease-back rate, sticky surfaces |
| Drive | thrust, top speed, drag (linear, quadratic), lateral grip, airbrake drag and yaw |
| Air | angular damping, auto-level rate, air steering |
| Walls | restitution, friction, crash threshold |
| Landing | hard-landing threshold, crash tolerance (angle) |
| Rest | hold speed, hold friction (as today's rest hold) |
| Surfaces (per piece) | friction, boost, bounce, gravity mode |

### How an engine drives it

Two modes, both behind one engine setting (`physics = game | core`):

1. **Collision only.** The game keeps its physics; the core answers its queries. This is what `wo3_room.cpp` does by
   hand now (ground under the ship, the walls that meet it), and what Mario Kart Wii does by building a KCL. Moving
   those queries into the core shares the scan handling, edge smoothing, noise and grid across engines, and the tests
   can use the same queries (`ramp.rs` has its own ground probe today).
2. **The core moves the ship.** Each frame the engine reads the pad, steps the core's body, and writes the result
   into the game's ship before its update:
   - position (`+0x58`) and last position (`+0x18`), velocity (`+0x68`, the game's units, x32);
   - the heading and angles (`+0x11E..+0x124`), from which the game builds its basis;
   - the section, as now, so the game's camera and checks are content.

   The game's own ship physics is skipped: the player's update function (`0x80119D50`), or the hover and wall parts
   of it, behind a replace hook (as the library does for the vsync wait and the scene draw). Speed and thrust stay
   in the ship's fields, so the game's engine sound and HUD read them as they do. The core's events drive the game's
   own effects through its functions (a wall hit's sound, a crash's). The model is the host's already (drawn from the
   game's model data), and its poses come from the core's body.

   It fits the engine's pipelined step: between one frame's end and the next's start the game is still. The Rust
   side reads the pad, steps the core, writes the ship, and starts the frame.

### Testing

The replay suite is the contract. `ramp.rs`, `room_dump.rs`, `corner.rs` and `free_flight.rs` run against either
mode, and the core is done when it passes them all with no thresholds of the kind listed above. The dumps the app
writes (`dump_stage`) are the regression set: every headset bug report should come with one.

## Migration

1. **Now**: the game's physics, with the rules above; the ramp suite in place.
2. **Collision first**: the collision world and its queries in the core; the library's room code calls them (over a
   C ABI, or from the Rust side between frames); the tests use them. The behaviour is unchanged and the replay suite
   proves it.
3. **The body**: the hover vehicle in the core, used in "core" mode for one engine (this one: its ship is simplest),
   and compared with the game's physics on the same replays.
4. **Gravity along normals**: once a course piece needs it, the core is the only mode for that piece.

# Elden Ring combat for Skyrim

An SKSE plugin that runs Elden Ring's player combat rules inside Skyrim SE/AE.
That covers rolls on release, sprint on hold, i-frames, attack chains with
their real cancel windows, charged heavies, guard counters and stamina costs.
It's built to sit on top of a big modlist (Gate to Sovngarde) without breaking it.

The combat logic is the engine-free simulation from
[Funny-Bones/ELDEN-RING-Combat-Rewrite](https://github.com/Funny-Bones/ELDEN-RING-Combat-Rewrite)
(MIT, see `third_party/`), taken from commit `177a3b5`.

## How it plays

- **Combat mode is on only while your weapon is drawn** and no menu is open.
  When you sheathe it, Skyrim and your other mods behave exactly as before.
- **While it's on, the plugin handles dodging, sprinting, jumping and attacks.**
  Skyrim's own sprint, jump and attack/block handlers are paused for that time.
  Each action the core starts plays through the animation event set in
  `ERCombat.ini`. By default those events drive TK Dodge and MCO/ADXP.
- **Activate, movement, shouts, sneaking and menus always stay with Skyrim**, so
  doors, loot and dialogue keep working mid-fight.
- **I-frames come from the core's real roll timings.** They're set through the
  `bIframeActive` / `bInIframe` graph variables.

## Layout

| Path | What it is |
|---|---|
| `core/` | Rust library `er_combat_core.dll`. `src/sim/` is the upstream simulation (Bevy math swapped for `glam`). `src/ffi.rs` is the C interface, in Skyrim units and axes. |
| `core/include/er_combat.h` | The C header for that interface. Tests check that it matches the Rust structs byte for byte. |
| `plugin/` | The SKSE plugin (C++, CommonLibSSE-NG). It hooks the player's update, reads input and plays the core's actions. |
| `plugin/dist/` | What goes in the mod archive: `SKSE/Plugins/ERCombat.dll`, `er_combat_core.dll` and `ERCombat.ini`. |
| `docs/PLAN.md` | The design and build order. |

## Building (Windows)

You need Rust, Visual Studio 2022 with C++, CMake, Ninja and vcpkg (with `VCPKG_ROOT` set).

```powershell
cd core
cargo build --release          # er_combat_core.dll + .dll.lib
cd ..\plugin
cmake --preset release
cmake --build --preset release # ERCombat.dll, both DLLs copied to dist\
```

Then install `plugin/dist` as a mod in Vortex.

## Elden Ring data

The action table (timings, i-frames, cancel windows, root motion) is generated
from **your own copy of Elden Ring** and is never committed. Without it, the
core builds against a stand-in (`core/src/sim/extracted_stub.rs`) so everything
compiles and the tests run. To use the real data:

1. Follow upstream's README Setup (unpack with UXM and WitchyBND, then run
   `python tools/setup.py`).
2. Copy its generated `src/sim/extracted.rs` to `core/src/sim/extracted.rs`
   and rebuild. `build.rs` notices it and switches over automatically.

The DLL from CI (or any build without `extracted.rs`) loads the real table at
run time instead, from `ERCombat.erdata` next to `ERCombat.dll`. To make that
file, with `core/src/sim/extracted.rs` in place:

```
cd core
cargo run --release --bin er_dump -- ERCombat.erdata
```

and copy `ERCombat.erdata` into `Data/SKSE/Plugins` (or the mod's
`SKSE/Plugins`). `ERCombat.log` says whether it loaded; without it the core
runs on the stand-in. To run the stand-in's tests while `extracted.rs` is
present, set `ER_COMBAT_STUB=1`.

Please keep generated data out of forks and uploads, as upstream asks.

## Status

- [x] Core as a library with a C interface; 9 tests (`cd core && cargo test`)
- [x] SKSE bridge scaffold: combat mode, input, action → animation event, i-frames
- [ ] First in-game build and test on 1.6.1170
- [ ] Lock-on target from True Directional Movement
- [ ] Gamepad bindings
- [ ] Core hit detection against Skyrim actors (phase B)
- [ ] Elden Ring animations converted from your own game files (phase B)

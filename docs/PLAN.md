# Skyrim × Elden Ring combat: our own plugin

**Aim**
- **Act as:** Skyrim SKSE plugin developer
- **Input:** the MIT-licensed combat core from [Funny-Bones/ELDEN-RING-Combat-Rewrite](https://github.com/Funny-Bones/ELDEN-RING-Combat-Rewrite), Skyrim SE/AE, and the Gate to Sovngarde modlist (GTS)
- **Mission:** play Skyrim with Elden Ring movement and combat in third person, while doors, loot, NPCs, menus and the rest of Gate to Sovngarde keep working

"GTS" is **Gate to Sovngarde**, the Wabbajack modlist Joel has installed. We target its exact game version and play alongside its combat, camera and HUD mods.

---

## 0. What the video actually shows (from Joel's Gemini notes, 2026-10-09)

The evidence suggests the video is **not a mod running inside Skyrim**. More likely, Skyrim's world was loaded into Funny-Bones' own Rust engine:
- **Performance:** locked at 240 FPS, and described as "better than a pass-through mod".
- **HUD and camera:** Skyrim's HUD is gone, and the camera is neither vanilla nor SmoothCam.
- **Player model:** the Skyrim player model was "rigged by AI" onto Elden Ring's skeleton, which causes shoulder clipping. A real Skyrim mod wouldn't need that.
- **Missing gameplay:** no menus, looting or dialogue are shown, and the creator talks about rebuilding the enemy AI.

This is inferred from a summary, not watched first-hand. If it's right, Gate to Sovngarde's mods, other SKSE mods and the overworld can't work in that version. That's the main reason to build our plugin inside real Skyrim.

The creator also said: *"if anyone wants to really work on this and integrate it well into Skyrim just let me know."* We can offer them this project in chasm's Discord ("share your projects" forum).

## 1. How it fits together

```
 Skyrim (SKSE)                               Rust core (DLL)
┌──────────────────────────────┐         ┌──────────────────────────────┐
│ C++ bridge plugin            │  input  │ fork of src/sim/             │
│  - reads keys/pad            │ ──────► │  - player state machine      │
│  - reads player pos, scale,  │  world  │  - timings, i-frames,        │
│    ground, nearby enemies    │ ──────► │    stamina, chains, hit-stop │
│  - applies movement, plays   │ ◄────── │  - says: move by X, face Y,  │
│    animations, deals damage  │ output  │    play action Z, hit lands  │
└──────────────────────────────┘         └──────────────────────────────┘
```

- **Rust core.** We fork the repo's `src/sim/`, which is already engine-free and uses only vector math. It gets built as a DLL with a small C interface: `create`, `step(input, world) -> output`, `destroy`.
- **C++ bridge.** This is an SKSE plugin built on CommonLibSSE-NG, so one build works on both SE and AE. Every frame it collects input and game state, steps the core, and applies the result to the player.
- **Skyrim does collision.** The repo's toy level (boxes on a flat floor) is replaced. The core only says how far to move, and Skyrim's own character controller handles walls, slopes and stairs.
- **Units.** The core works in meters. Skyrim uses about 70 units per meter, and the bridge converts in both directions.

## 2. The overworld fix (your request)

Combat mode is **on only while a weapon is drawn**. With the weapon sheathed, everything is plain Skyrim.

While a weapon is drawn:
- **Activate (E / A button)** goes through to Skyrim whenever the core isn't in the middle of an attack, roll or stagger. That covers doors, looting, talking and picking things up.
- **Menus, map, inventory, favorites and shouts** always go through.
- **Dialogue or any menu** pauses the core so nothing fires behind the menu.
- **Keybinds that clash** with Skyrim (Space for jump, F, Q) can be remapped in an INI. We'll ship sensible defaults.

## 3. Gate to Sovngarde compatibility

Joel's install is managed with **Vortex**, and the game version it plays on is **1.6.1170** (the runtime swapper switches the game from 1.7.104 to it). These mods do the same jobs as ours (from `joel-setup/Plugins.txt`):

| Job | Mods in Joel's list |
|---|---|
| Movement and lock-on | True Directional Movement, Reforged Directional Combat |
| Camera | SmoothCam |
| Attacks and combos | MCO/ADXP, SCAR, Smooth Moveset, Sekiro Combat II |
| Hit detection | Precision |
| Dodging | TK Dodge, Dynamic Dodge Shot |
| Block, parry and stagger | Elden Parry, Elden Counter, Simple Timed Block, Chocolate Poise Reforged, Blade and Blunt |
| Stamina | For Honor Stamina System, Blade and Blunt |
| HUD | TrueHUD |

The general rules are:
- **Same game version.** We build against the Skyrim and SKSE version the list pins, using CommonLibSSE-NG.
- **One owner per job while combat mode is on.** Our plugin owns movement, dodging, attacks and lock-on. Anything else in the list that does those jobs (directional movement or lock-on such as TDM, attack and combo frameworks such as MCO/BFCO, dodge mods, weapon-collision mods such as Precision) gets paused, or bypassed by ours, only while combat mode is on. When the weapon is sheathed, the list plays exactly as before.
- **Keep the list's camera and HUD where we can.** If SmoothCam is in the list, we feed it the lock-on target rather than replacing the camera. If TrueHUD is in the list, we show stamina and the lock-on marker through it.
- **Damage goes through Skyrim.** Hits use Skyrim's normal hit and damage path, so the list's perks, difficulty settings, enemy AI and gore keep working.
- **A shareable patch.** We ship this as a separate mod plus a small patch for Gate to Sovngarde, so nothing in the Wabbajack install itself gets edited.

## 4. Animations

This decides what we can share publicly.
- **Personal setup, same as the repo (default).** We ship tools, not animations. Each player runs our converter on their own Elden Ring copy, and it retargets the animations onto Skyrim's skeleton and writes `.hkx` files. Nothing from Elden Ring goes up on Nexus, just as the original repo ships no game data.
- **Shareable fallback.** If a player doesn't own Elden Ring, the core drives Skyrim combat animation packs (MCO/BFCO) through Open Animation Replacer. It looks less exact but needs no Elden Ring files.

## 5. Build order

1. **Fork the core and add the C interface.** Swap the Bevy dependency for plain `glam`, so the DLL doesn't pull in a whole game engine. Keep the repo's headless tests passing.
2. **Bridge v0: movement only.** Walk, run, sprint, roll and backstep, using Skyrim's existing animations.
3. **Overworld rules.** Combat mode only while a weapon is drawn, plus activate passthrough.
4. **Combat.** Light, heavy and charged attacks with real hit detection against Skyrim NPCs, plus stamina, guarding and hit-stop.
5. **Lock-on and camera.** Ideally this plays nicely with SmoothCam, or replaces it.
6. **Animation converter.** Elden Ring animations retargeted to Skyrim's skeleton as `.hkx`.
7. **Gate to Sovngarde pass.** The rules from section 3, tested in Joel's installed list.

## 6. What only you can provide

- **A GitHub repo.** I can't create one from here. Make an empty repo (for example `skyrim-er-combat`) and connect it, and I'll push everything there.
- **Your PC for building and testing.** The plugin needs Windows with Visual Studio, Skyrim SE/AE with SKSE, Elden Ring (for the data the core needs) and Gate to Sovngarde. I can run builds and tests there through a session on your device once you allow it.

## 7. Full Elden Ring takeover (Joel, 2026-10-11)

Joel's direction: pressing **Ä** switches Skyrim's whole combat and movement over to Elden Ring's, with every moveset, on every Skyrim weapon type. Every other combat and movement mod steps aside while it is on. 360° movement stays, and Elden Ring's own movement provides it anyway.

### What "switching the other mods off" can mean

SKSE plugins and behavior patches (MCO, TK Dodge, Precision, TDM, Elden Parry, OAR submods) can't be unloaded while the game runs. What we can do is make sure **none of them gets anything to work with** while Elden Ring mode is on:

- **We drive the player's pose ourselves.** Every frame, after Skyrim's animation update, the plugin writes the retargeted Elden Ring pose straight onto the player's skeleton. The behavior graph underneath keeps idling, so MCO, BFCO, TK Dodge, SCAR and OAR replacers never see an attack or dodge event, and they never play anything.
- **We own movement.** The core's root motion moves the player, so Skyrim's own movement, and TDM's, are bypassed. TDM's API (`RequestDisableDirectionalMovement`, yaw control) switches it off for the duration, and Elden Ring's own lock-on and 360° movement take its place. The camera stays with SmoothCam or vanilla.
- **We own hits.** The core's weapon sweeps decide hits (phase B). Damage still goes through Skyrim's hit path, so perks, difficulty and gore keep working. Precision, Elden Parry and the block mods never fire because no Skyrim attack is ever started. Hits on the player go through the core first (i-frames, guard, poise).
- **Off means off.** Pressing Ä again hands everything back: graph events flow again, TDM is re-enabled, and the list plays exactly as before.

### Weapons

Upstream already has 24 Elden Ring classes, each one- and two-handed, with power stance and left-hand chains. Skyrim's weapon types map onto them:

| Skyrim | Elden Ring class |
|---|---|
| Dagger | Dagger |
| Sword | Longsword |
| War Axe | Battle Axe |
| Mace | Club |
| Greatsword | Claymore |
| Battleaxe | Greataxe |
| Warhammer | Great Hammer |
| Hand to hand | Fist |
| Shield / Torch (left hand) | Shield / Torch |
| Modded types by keyword (Animated Armoury and similar): katana, rapier, spear, halberd, scythe, claw, whip, quarterstaff | Uchigatana, Rapier, Short Spear, Halberd, Reaper, Claw, Whip, Twinblade |

Bows, crossbows, staves and spells aren't in upstream. With one of those equipped, Elden Ring mode passes everything through to Skyrim.

### What it needs from Joel's Elden Ring

Running upstream's `tools/setup.py` on his own copy creates two files:
- `src/sim/extracted.rs`: the action table (timings, i-frames, cancel windows, root motion, stamina). The core is built against it.
- `assets/player_anims.bin`: the baked animations on Elden Ring's skeleton. Our converter retargets them once onto Skyrim's skeleton and writes `Data/SKSE/Plugins/ERCombat/anims.bin` on Joel's PC.

Neither file is ever committed or uploaded, so CI keeps building against the stub. The real core DLL is built from Joel's `extracted.rs` outside the public repo.

### New build order

1. Real action table: build the core with Joel's `extracted.rs`.
2. Skeleton retarget: map Elden Ring's c0000 bones to Skyrim's, and write the converter.
3. Pose driver: sample the current action's animation in the core, and write bones each frame in the bridge. Root motion moves the player.
4. Isolation switches: stop graph events, turn TDM off through its API, hold Skyrim's attack/block/sprint/jump handlers.
5. Weapon mapping from the table above, including two-handing and power stance.
6. Hits both ways through the core, with damage through Skyrim.
7. Lock-on target fed to SmoothCam and TrueHUD.

## 8. Features after the takeover (Joel, 2026-10-11)

- **Enemies fight the Elden Ring way too.** Humanoid NPCs reuse the player movesets (same skeleton) with ER timings, rolls and posture. Creatures keep Skyrim animations but get posture and stagger.
- **Posture break like Sekiro.** When posture breaks: guard break plus the Elden Ring critical stab. At zero HP: Skyrim killmove.
- **Ashes of War.** Each weapon class gets a default skill on one key, paid with Magicka as FP. A lesser power or MCM page swaps the skill.
- **Boss health bar.** Bosses and dragons get an ER-style bar (TrueHUD boss bar). Music already comes from the modlist.
- Not now: mounted combat. Flasks: Joel already has a mod for that.

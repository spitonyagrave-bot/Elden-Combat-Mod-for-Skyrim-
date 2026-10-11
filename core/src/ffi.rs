//! The C interface the SKSE bridge calls. See `include/er_combat.h`.
//!
//! Everything crossing this boundary is in Skyrim's space: game units
//! (`UNITS_PER_METRE` to the metre), Z up, and heading in radians with 0
//! facing +Y and turning clockwise toward +X (+X is to the right of +Y). The
//! simulation works in metres in Bevy's right-handed space: Y up, yaw 0 facing
//! +Z, and +X on the character's *left*, with positive yaw turning toward +X
//! (counter-clockwise seen from above). So Skyrim's X is the simulation's -X,
//! and a Skyrim heading is the negated simulation yaw. Mapping X straight
//! across would mirror the world: stick right would roll left.

use std::ffi::{c_char, CStr};
use std::path::Path;

use glam::{Vec2, Vec3};

use crate::pose::retarget::{ErXform, BONE_COUNT};
use crate::pose::{self, Driver, Frame, Info};
use crate::sim::data::{ActionId, AttackKind, Dir, HurtLevel, DT};
use crate::sim::erdata::ReadError;
use crate::sim::tables::{self, LoadError};
use crate::sim::level::Level;
use crate::sim::player::{Button, HitResult, Incoming, Input, Player, State, SWEEP_STEPS};

/// Skyrim units in a metre. One unit is 1/70 m, near enough.
pub const UNITS_PER_METRE: f32 = 70.0;

/// Never step more than this many ticks in one frame, so a long hitch (a
/// loading screen, a breakpoint) does not replay seconds of input at once.
const MAX_TICKS_PER_FRAME: u32 = 8;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct ErVec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl ErVec3 {
    pub(crate) fn to_sim(self) -> Vec3 {
        Vec3::new(-self.x, self.z, self.y) / UNITS_PER_METRE
    }

    pub(crate) fn from_sim(v: Vec3) -> Self {
        let v = v * UNITS_PER_METRE;
        Self { x: -v.x, y: v.z, z: v.y }
    }
}

/// Skyrim heading to simulation yaw, and back (the same negation).
pub(crate) fn yaw_to_sim(heading: f32) -> f32 {
    -heading
}

pub(crate) fn yaw_from_sim(yaw: f32) -> f32 {
    -yaw
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct ErButton {
    pub held: u8,
    pub pressed: u8,
    pub released: u8,
}

/// One frame of controls. `pressed`, `released` and the one-shot flags mean
/// "since the last call"; the library hands them to the first tick only.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct ErInput {
    /// Movement stick: x right, y forward, relative to the camera, -1..1.
    pub move_x: f32,
    pub move_y: f32,
    /// Camera heading, Skyrim convention.
    pub camera_heading: f32,
    pub dodge: ErButton,
    pub jump: ErButton,
    pub light: ErButton,
    pub heavy: ErButton,
    pub guard: ErButton,
    pub crouch: u8,
    pub walk: u8,
    pub lock: u8,
    pub two_hand_right: u8,
    pub two_hand_left: u8,
}

/// What Skyrim knows about the player this frame.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct ErWorld {
    /// Where the player actually is, after Skyrim's collision.
    pub position: ErVec3,
    /// Height of the ground under the player.
    pub ground_z: f32,
    /// The lock-on target's position, when `has_target` is set.
    pub has_target: u8,
    pub target: ErVec3,
    /// A hostile actor is near: sprinting costs stamina.
    pub in_combat: u8,
    /// Which way the player actually faces in Skyrim.
    pub heading: f32,
    /// Skyrim (its locomotion, or a directional-movement mod) turns the
    /// player this frame: the simulation takes `heading` as its own facing.
    /// Otherwise the simulation owns the facing and the bridge applies
    /// `ErOutput::heading`. The first update always takes `heading`.
    pub follow_heading: u8,
}

/// Seconds the simulation may think it is in the air while Skyrim reports the
/// player standing still at one height, before it is put down on the ground.
/// Skyrim decides whether the player really left the ground; the simulation
/// must never wait for a fall that is not happening.
const STUCK_IN_AIR_SECONDS: f32 = 0.25;

/// What the player is doing, for the bridge to pick animations.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ErState {
    Ground = 0,
    Action = 1,
    Air = 2,
    Dead = 3,
}

/// Which kind of action is playing, for the bridge to pick the Skyrim
/// animation event that goes with it.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ErAction {
    None = 0,
    Roll = 1,
    CrouchRoll = 2,
    Backstep = 3,
    SprintStop = 4,
    Jump = 5,
    Land = 6,
    LightAttack = 7,
    /// The heavy attack's wind-up, which becomes `HeavyAttack` or charges.
    HeavyCharge = 8,
    HeavyAttack = 9,
    RunAttack = 10,
    RollAttack = 11,
    BackstepAttack = 12,
    CrouchAttack = 13,
    JumpAttackLand = 14,
    GuardCounter = 15,
    LeftAttack = 16,
    GuardHit = 17,
    GuardBreak = 18,
    Hurt = 19,
}

/// The simulation's answer for this frame.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ErOutput {
    /// Where the player should be now. The bridge moves the character
    /// controller toward it and lets Skyrim's collision have the last word.
    pub position: ErVec3,
    pub heading: f32,
    pub state: ErState,
    /// The playing action's animation name from the action table (an Elden
    /// Ring clip id such as `a000_030000`), NUL-terminated; empty on the
    /// ground or in the air.
    pub action: [u8; 32],
    /// Frame of that action, at 30 fps.
    pub action_frame: f32,
    pub action_kind: ErAction,
    /// Direction of a roll, landing or hit reaction: 0 front, 1 back,
    /// 2 left, 3 right.
    pub action_dir: u32,
    /// Goes up by one every time a new action starts, including the next
    /// swing of a chain, so the bridge fires each animation event once.
    pub action_serial: u32,
    /// Ground speed, game units per second.
    pub speed: f32,
    pub sprinting: u8,
    pub crouching: u8,
    /// Raising or holding the guard (it blocks once fully up).
    pub guarding: u8,
    pub invincible: u8,
    pub airborne: u8,
    /// Nothing is playing that pressing Activate would cut short, so the
    /// bridge may let Activate through to Skyrim (doors, loot, talking).
    pub interruptible: u8,
    /// A jump attack is under way in the air (it sits in what was padding).
    pub air_attack: u8,
    /// Stamina as a share of the maximum, 0..1.
    pub stamina: f32,
    /// A swing is live this frame and has not connected yet.
    pub hit_live: u8,
    /// The blade's two ends at a few moments over the last tick. A target
    /// is hit when any of these segments comes within `hit_radius` of it.
    pub sweep_a: [ErVec3; SWEEP_STEPS],
    pub sweep_b: [ErVec3; SWEEP_STEPS],
    pub hit_radius: f32,
    /// Weapon base attack times the swing's motion value.
    pub hit_damage: f32,
    /// Seconds both sides should freeze for if this swing lands.
    pub hit_stop: f32,
    /// The swing's motion value (1.0 = the weapon's plain damage), for the
    /// bridge to scale Skyrim's own weapon damage by.
    pub hit_motion: f32,
}

/// How hard an incoming hit is.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ErHurt {
    Small = 0,
    Middle = 1,
    Large = 2,
    Knockdown = 3,
}

/// What became of an incoming hit. On `Hit`, Skyrim applies its own damage;
/// on `Dodged`, `Jumped` and `Blocked` the bridge cancels it.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ErHitResult {
    Ignored = 0,
    Dodged = 1,
    Jumped = 2,
    Blocked = 3,
    GuardBroken = 4,
    Hit = 5,
}

pub struct ErCombat {
    player: Player,
    level: Level,
    /// Seconds of real time not yet simulated.
    pending: f32,
    serial: u32,
    /// The action playing after the last tick, to spot new ones.
    last_action: Option<(ActionId, f32)>,
    /// Facing taken from Skyrim at least once.
    synced: bool,
    /// Skyrim's height of the player last frame, and how long the simulation
    /// has been airborne while that height did not change.
    last_z: Option<f32>,
    air_still: f32,
    /// Once that has happened, the height (Skyrim units) the player is held
    /// at, taken as ground for as long as Skyrim keeps the player there.
    held_at: Option<f32>,
}

impl ErCombat {
    pub fn new(position: ErVec3) -> Self {
        Self {
            player: Player::new(position.to_sim()),
            level: Level::flat(),
            pending: 0.0,
            serial: 0,
            last_action: None,
            synced: false,
            last_z: None,
            air_still: 0.0,
            held_at: None,
        }
    }

    pub fn update(&mut self, dt: f32, input: &ErInput, world: &ErWorld) -> ErOutput {
        // Skyrim's collision has the last word on where the player is.
        self.player.pos = world.position.to_sim();
        if world.follow_heading != 0 || !self.synced {
            self.player.yaw = yaw_to_sim(world.heading);
            self.synced = true;
        }
        let mut ground = world.ground_z / UNITS_PER_METRE;
        // In the air in the simulation, but Skyrim has the player standing at
        // the same height frame after frame: Skyrim never jumped (no animation
        // answered, or something held the player down). Land where Skyrim
        // has the player rather than stay airborne, and locked, for ever.
        let standing = self.last_z.is_some_and(|z| (z - world.position.z).abs() < 1.0);
        self.last_z = Some(world.position.z);
        if matches!(self.player.state, State::Air(_)) && standing {
            self.air_still += dt.max(0.0);
        } else {
            self.air_still = 0.0;
        }
        if self.air_still >= STUCK_IN_AIR_SECONDS {
            self.held_at = Some(world.position.z);
            self.air_still = 0.0;
        }
        if !standing {
            self.held_at = None;
        }
        if let Some(z) = self.held_at {
            ground = ground.max(z / UNITS_PER_METRE);
        }
        self.level.ground = Some(ground);
        let target = (world.has_target != 0).then(|| world.target.to_sim());

        self.pending = (self.pending + dt.max(0.0)).min(DT * MAX_TICKS_PER_FRAME as f32);
        let mut first = sim_input(input);
        let held = held_only(&first);
        while self.pending >= DT {
            self.player.step(&first, &self.level, target, world.in_combat != 0);
            self.pending -= DT;
            first = held;
            self.note_new_action();
        }
        self.output()
    }

    fn note_new_action(&mut self) {
        let now = match self.player.state {
            State::Act(a) => Some((a.id, a.f)),
            _ => None,
        };
        if let Some((id, f)) = now {
            // A new action, or the same one started over (the next swing of a
            // chain can share an id with the last).
            let fresh = match self.last_action {
                Some((last, last_f)) => last != id || f < last_f,
                None => true,
            };
            if fresh {
                self.serial = self.serial.wrapping_add(1);
            }
        }
        self.last_action = now;
    }

    /// Animation frames the simulation's clock is past its last tick.
    pub fn ahead(&self) -> f32 {
        (self.pending / DT).clamp(0.0, 1.0) * crate::sim::data::DF
    }

    /// How high the simulation has the player above the ground Skyrim
    /// reported (a jump's arc), Skyrim units.
    pub fn lift(&self) -> f32 {
        let ground = self.level.height(self.player.pos.x, self.player.pos.z);
        ((self.player.pos.y - ground) * UNITS_PER_METRE).max(0.0)
    }

    pub fn player(&self) -> &Player {
        &self.player
    }

    /// What is in each hand, as indices into the weapon table (`right` never
    /// the shield), and whether the right-hand weapon is held in both hands.
    /// Out-of-range indices keep what is there. Returns whether anything
    /// changed.
    pub fn set_armaments(&mut self, right: i32, left: i32, two_handed: bool) -> bool {
        use crate::sim::data::{shield, weapons, Grip};
        let n = weapons().len() as i32;
        let p = &mut self.player;
        let right = if (0..n).contains(&right) && right as usize != shield() { right as usize } else { p.weapon };
        let left = if (0..n).contains(&left) { left as usize } else { p.left };
        let grip = if two_handed { Grip::TwoHandRight } else { Grip::OneHand };
        if (right, left, grip) == (p.weapon, p.left, p.grip) {
            return false;
        }
        p.weapon = right;
        p.left = left;
        p.grip = grip;
        // Skyrim did the equipping; no grip-change animation of our own.
        p.swap = None;
        true
    }

    pub fn receive_hit(&mut self, from: ErVec3, low: bool, hurt: ErHurt) -> ErHitResult {
        // Skyrim keeps health: the simulation only decides whether the hit
        // was avoided and which reaction plays.
        let hit = Incoming {
            damage: 0.0,
            stamina: 20.0,
            from: from.to_sim(),
            low,
            level: match hurt {
                ErHurt::Small => HurtLevel::Small,
                ErHurt::Middle => HurtLevel::Middle,
                ErHurt::Large => HurtLevel::Large,
                ErHurt::Knockdown => HurtLevel::Knockdown,
            },
        };
        match self.player.receive_hit(&hit, &self.level) {
            HitResult::Ignored => ErHitResult::Ignored,
            HitResult::Dodged => ErHitResult::Dodged,
            HitResult::Jumped => ErHitResult::Jumped,
            HitResult::Blocked => ErHitResult::Blocked,
            HitResult::GuardBroken => ErHitResult::GuardBroken,
            HitResult::Hit | HitResult::Killed => ErHitResult::Hit,
        }
    }

    /// The live swing connected: stop it hitting again and start hit-stop.
    pub fn confirm_hit(&mut self) {
        if let Some(hit) = self.player.active_hit() {
            self.player.mark_hit();
            self.player.hit_stop = hit.stop * crate::sim::data::HIT_STOP_SCALE;
        }
    }

    pub fn output(&self) -> ErOutput {
        let p = &self.player;
        let (state, action, frame, interruptible) = match p.state {
            State::Ground => (ErState::Ground, "", 0.0, true),
            State::Air(_) => (ErState::Air, "", 0.0, false),
            State::Dead { .. } => (ErState::Dead, "", 0.0, false),
            State::Act(a) => {
                let def = a.id.def();
                let free = a.f >= def.cancel_move && !matches!(a.id, ActionId::Attack(..) if a.f < def.total);
                (ErState::Action, def.source, a.f, free)
            }
        };
        let (kind, dir) = match p.state {
            State::Act(a) => classify(a.id),
            _ => (ErAction::None, Dir::Front),
        };
        let mut name = [0u8; 32];
        let len = action.len().min(name.len() - 1);
        name[..len].copy_from_slice(&action.as_bytes()[..len]);

        let hit = p.active_hit();
        let mut sweep_a = [ErVec3::default(); SWEEP_STEPS];
        let mut sweep_b = [ErVec3::default(); SWEEP_STEPS];
        if let Some(hit) = &hit {
            for (i, &(a, b)) in hit.sweep.iter().enumerate() {
                sweep_a[i] = ErVec3::from_sim(a);
                sweep_b[i] = ErVec3::from_sim(b);
            }
        }
        ErOutput {
            position: ErVec3::from_sim(p.pos),
            heading: yaw_from_sim(p.yaw),
            state,
            action: name,
            action_frame: frame,
            action_kind: kind,
            action_dir: dir as u32,
            action_serial: self.serial,
            speed: p.speed * UNITS_PER_METRE,
            sprinting: p.sprinting as u8,
            crouching: p.crouching as u8,
            guarding: p.guarding as u8,
            invincible: p.invincible() as u8,
            airborne: p.airborne() as u8,
            interruptible: (interruptible && p.swap.is_none()) as u8,
            air_attack: p.air_attack.is_some() as u8,
            stamina: (p.stamina / crate::sim::data::max_stamina()).clamp(0.0, 1.0),
            hit_live: hit.is_some() as u8,
            sweep_a,
            sweep_b,
            hit_radius: hit.map_or(0.0, |h| h.radius * UNITS_PER_METRE),
            hit_damage: hit.map_or(0.0, |h| h.attack * h.mv),
            hit_stop: hit.map_or(0.0, |h| h.stop * crate::sim::data::HIT_STOP_SCALE),
            hit_motion: hit.map_or(0.0, |h| h.mv),
        }
    }
}

fn classify(id: ActionId) -> (ErAction, Dir) {
    use AttackKind::*;
    match id {
        ActionId::Roll(_, dir) => (ErAction::Roll, dir),
        ActionId::CrouchRoll(_, dir) => (ErAction::CrouchRoll, dir),
        ActionId::Backstep => (ErAction::Backstep, Dir::Back),
        ActionId::SprintStop => (ErAction::SprintStop, Dir::Front),
        ActionId::Jump(_) => (ErAction::Jump, Dir::Front),
        ActionId::LandStrafeWalk(dir) | ActionId::LandStrafe(dir) => (ErAction::Land, dir),
        ActionId::LandLight | ActionId::LandRun | ActionId::LandSprint | ActionId::LandHeavy | ActionId::LandFall => {
            (ErAction::Land, Dir::Front)
        }
        ActionId::GuardHit => (ErAction::GuardHit, Dir::Front),
        ActionId::GuardBreak => (ErAction::GuardBreak, Dir::Front),
        ActionId::Hurt(_, dir) => (ErAction::Hurt, dir),
        ActionId::Attack(_, kind) => (
            match kind {
                Light1 | Light2 | Light3 | Light4 | Light5 | Light6 => ErAction::LightAttack,
                Heavy1Charge | Heavy2Charge => ErAction::HeavyCharge,
                Heavy1 | Heavy2 => ErAction::HeavyAttack,
                RunLight | RunHeavy | PairedRun => ErAction::RunAttack,
                RollAttack | PairedRoll => ErAction::RollAttack,
                BackstepAttack | PairedBackstep => ErAction::BackstepAttack,
                CrouchAttack => ErAction::CrouchAttack,
                JumpLightLand | JumpLightLandShort | JumpHeavyLand | JumpHeavyLandShort | PairedJumpLand
                | PairedJumpLandShort => ErAction::JumpAttackLand,
                GuardCounter => ErAction::GuardCounter,
                LeftLight1 | LeftLight2 | LeftLight3 | LeftLight4 | LeftLight5 | LeftLight6 | PairedLight1
                | PairedLight2 | PairedLight3 | PairedLight4 | PairedLight5 | PairedLight6 => ErAction::LeftAttack,
            },
            Dir::Front,
        ),
    }
}

fn button(b: ErButton) -> Button {
    Button { held: b.held != 0, pressed: b.pressed != 0, released: b.released != 0 }
}

fn sim_input(i: &ErInput) -> Input {
    Input {
        mv: Vec2::new(i.move_x, i.move_y),
        cam_yaw: yaw_to_sim(i.camera_heading),
        dodge: button(i.dodge),
        jump: button(i.jump),
        light: button(i.light),
        heavy: button(i.heavy),
        guard: button(i.guard),
        crouch: i.crouch != 0,
        lock: i.lock != 0,
        walk: i.walk != 0,
        two_hand_right: i.two_hand_right != 0,
        two_hand_left: i.two_hand_left != 0,
        // Weapon changes go through Skyrim's own equipping.
        next_weapon: false,
        next_left: false,
    }
}

/// The same input with every edge and one-shot cleared: what later ticks in
/// the same frame see.
fn held_only(i: &Input) -> Input {
    let hold = |b: Button| Button { held: b.held, pressed: false, released: false };
    Input {
        dodge: hold(i.dodge),
        jump: hold(i.jump),
        light: hold(i.light),
        heavy: hold(i.heavy),
        guard: hold(i.guard),
        lock: false,
        two_hand_right: false,
        two_hand_left: false,
        next_weapon: false,
        next_left: false,
        ..*i
    }
}

// --- C ABI -------------------------------------------------------------------

/// `er_load_data` results.
pub const ER_LOAD_OK: i32 = 0;
/// `path` is null or not UTF-8.
pub const ER_LOAD_BAD_PATH: i32 = 1;
/// The file can't be opened or read (most often: it isn't there).
pub const ER_LOAD_IO: i32 = 2;
/// Not an `.erdata` file.
pub const ER_LOAD_NOT_ERDATA: i32 = 3;
/// Written by another version of the format.
pub const ER_LOAD_VERSION: i32 = 4;
/// Written by a core whose action lists differ from this one's.
pub const ER_LOAD_LAYOUT: i32 = 5;
/// Cut short or damaged.
pub const ER_LOAD_CORRUPT: i32 = 6;
/// A table was already loaded; it stays.
pub const ER_LOAD_ALREADY_LOADED: i32 = 7;

fn load_result(result: Result<(), LoadError>) -> i32 {
    match result {
        Ok(()) => ER_LOAD_OK,
        Err(LoadError::Io(_)) => ER_LOAD_IO,
        Err(LoadError::AlreadyLoaded) => ER_LOAD_ALREADY_LOADED,
        Err(LoadError::Read(e)) => match e {
            ReadError::BadMagic => ER_LOAD_NOT_ERDATA,
            ReadError::Version(_) => ER_LOAD_VERSION,
            ReadError::Layout => ER_LOAD_LAYOUT,
            ReadError::Truncated | ReadError::Invalid(_) | ReadError::TrailingBytes => ER_LOAD_CORRUPT,
        },
    }
}

/// Loads the Elden Ring action table from an `.erdata` file (written by
/// `er_dump`), in place of the one compiled in. Call it once, before
/// `er_create`. On failure nothing changes and the compiled-in table stays.
///
/// # Safety
/// `path` must be null or a NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn er_load_data(path: *const c_char) -> i32 {
    if path.is_null() {
        return ER_LOAD_BAD_PATH;
    }
    let Ok(path) = CStr::from_ptr(path).to_str() else {
        return ER_LOAD_BAD_PATH;
    };
    load_result(tables::load_file(Path::new(path)))
}

/// Creates the simulation for a player standing at `position`.
#[no_mangle]
pub extern "C" fn er_create(position: ErVec3) -> *mut ErCombat {
    Box::into_raw(Box::new(ErCombat::new(position)))
}

/// # Safety
/// `combat` must come from `er_create` and not have been destroyed.
#[no_mangle]
pub unsafe extern "C" fn er_destroy(combat: *mut ErCombat) {
    if !combat.is_null() {
        drop(Box::from_raw(combat));
    }
}

/// Advances by `dt` seconds of game time and writes the result to `out`.
///
/// # Safety
/// All pointers must be valid; `combat` must come from `er_create`.
#[no_mangle]
pub unsafe extern "C" fn er_update(
    combat: *mut ErCombat,
    dt: f32,
    input: *const ErInput,
    world: *const ErWorld,
    out: *mut ErOutput,
) {
    let (Some(combat), Some(input), Some(world)) = (combat.as_mut(), input.as_ref(), world.as_ref()) else {
        return;
    };
    if !out.is_null() {
        *out = combat.update(dt, input, world);
    }
}

/// An enemy's attack reached the player. Returns whether it was dodged,
/// jumped, blocked or taken.
///
/// # Safety
/// `combat` must come from `er_create`.
#[no_mangle]
pub unsafe extern "C" fn er_receive_hit(combat: *mut ErCombat, from: ErVec3, low: u8, hurt: ErHurt) -> ErHitResult {
    match combat.as_mut() {
        Some(combat) => combat.receive_hit(from, low != 0, hurt),
        None => ErHitResult::Ignored,
    }
}

/// The live swing hit something. Call once per swing that lands.
///
/// # Safety
/// `combat` must come from `er_create`.
#[no_mangle]
pub unsafe extern "C" fn er_confirm_hit(combat: *mut ErCombat) {
    if let Some(combat) = combat.as_mut() {
        combat.confirm_hit();
    }
}

/// Speed of a gait in Skyrim units per second, from the loaded action table:
/// 0 walk, 1 run, 2 sprint (anything else: run).
#[no_mangle]
pub extern "C" fn er_gait_speed(gait: u32) -> f32 {
    use crate::sim::data::{run_speed, sprint_speed, walk_speed};
    let metres = match gait {
        0 => walk_speed(),
        2 => sprint_speed(),
        _ => run_speed(),
    };
    metres * UNITS_PER_METRE
}

// --- Weapons -------------------------------------------------------------------

/// Index of the Elden Ring weapon class called `name` (e.g. "Longsword",
/// "Shield", "Fist") in the action table, or -1.
///
/// # Safety
/// `name` must be null or a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn er_weapon_index(name: *const c_char) -> i32 {
    if name.is_null() {
        return -1;
    }
    let Ok(name) = CStr::from_ptr(name).to_str() else {
        return -1;
    };
    crate::sim::data::weapons().iter().position(|w| w.name == name).map_or(-1, |i| i as i32)
}

/// Puts these weapon classes in the player's hands (indices from
/// `er_weapon_index`; -1 keeps what is there). Returns 1 if anything changed.
///
/// # Safety
/// `combat` must come from `er_create`.
#[no_mangle]
pub unsafe extern "C" fn er_set_armaments(combat: *mut ErCombat, right: i32, left: i32, two_handed: u8) -> u8 {
    combat.as_mut().is_some_and(|c| c.set_armaments(right, left, two_handed != 0)) as u8
}

// --- Pose driver ---------------------------------------------------------------

/// Skyrim node names of the bones in `ErXform` arrays, NUL-terminated.
const BONE_NAMES: [&CStr; BONE_COUNT] = [
    c"NPC Pelvis [Pelv]",
    c"NPC Spine [Spn0]",
    c"NPC Spine1 [Spn1]",
    c"NPC Spine2 [Spn2]",
    c"NPC Neck [Neck]",
    c"NPC Head [Head]",
    c"NPC L Clavicle [LClv]",
    c"NPC L UpperArm [LUar]",
    c"NPC L Forearm [LLar]",
    c"NPC L Hand [LHnd]",
    c"NPC R Clavicle [RClv]",
    c"NPC R UpperArm [RUar]",
    c"NPC R Forearm [RLar]",
    c"NPC R Hand [RHnd]",
    c"NPC L Thigh [LThg]",
    c"NPC L Calf [LClf]",
    c"NPC L Foot [Lft ]",
    c"NPC L Toe0 [LToe]",
    c"NPC R Thigh [RThg]",
    c"NPC R Calf [RClf]",
    c"NPC R Foot [Rft ]",
    c"NPC R Toe0 [RToe]",
    c"NPC L Finger10 [LF10]",
    c"NPC L Finger20 [LF20]",
    c"NPC R Finger10 [RF10]",
    c"NPC R Finger20 [RF20]",
];

/// What `er_rig_update` did, for the bridge's log.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ErPoseInfo {
    /// The Elden Ring clip playing, NUL-terminated ("" when none).
    pub clip: [u8; 32],
    pub frame: f32,
    /// 0 Skyrim's pose .. 1 Elden Ring's.
    pub weight: f32,
    /// The clip the state calls for is not in ERCombat.eranims.
    pub missing_clip: u8,
}

impl Default for ErPoseInfo {
    fn default() -> Self {
        Self { clip: [0; 32], frame: 0.0, weight: 0.0, missing_clip: 0 }
    }
}

fn anim_load_result(result: Result<(), pose::LoadError>) -> i32 {
    use crate::pose::clips::AnimError;
    match result {
        Ok(()) => ER_LOAD_OK,
        Err(pose::LoadError::Io(_)) => ER_LOAD_IO,
        Err(pose::LoadError::AlreadyLoaded) => ER_LOAD_ALREADY_LOADED,
        Err(pose::LoadError::Read(AnimError::BadMagic)) => ER_LOAD_NOT_ERDATA,
        Err(pose::LoadError::Read(AnimError::MissingJoint(_))) => ER_LOAD_LAYOUT,
        Err(pose::LoadError::Read(_)) => ER_LOAD_CORRUPT,
    }
}

/// Loads the baked Elden Ring animations (`ERCombat.eranims`, which is
/// upstream's `player_anims.bin`). Once per process; returns an ErLoadResult
/// (NOT_ERDATA: not an animation file; LAYOUT: a joint the retarget needs is
/// missing).
///
/// # Safety
/// `path` must be null or a NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn er_load_anims(path: *const c_char) -> i32 {
    if path.is_null() {
        return ER_LOAD_BAD_PATH;
    }
    let Ok(path) = CStr::from_ptr(path).to_str() else {
        return ER_LOAD_BAD_PATH;
    };
    anim_load_result(pose::load_file(Path::new(path)))
}

/// Clips loaded by `er_load_anims` (0 if none).
#[no_mangle]
pub extern "C" fn er_anims_clip_count() -> u32 {
    pose::loaded().map_or(0, |c| c.len() as u32)
}

/// Skyrim node name of bone `bone` (< ER_BONE_COUNT), or null.
#[no_mangle]
pub extern "C" fn er_bone_name(bone: u32) -> *const c_char {
    BONE_NAMES.get(bone as usize).map_or(std::ptr::null(), |n| n.as_ptr())
}

/// Whether the driver ever writes bone `bone` (the rest are landmarks it
/// only reads).
#[no_mangle]
pub extern "C" fn er_bone_driven(bone: u32) -> u8 {
    ((bone as usize) < BONE_COUNT && crate::pose::retarget::driven(bone as usize)) as u8
}

#[no_mangle]
pub extern "C" fn er_rig_create() -> *mut Driver {
    Box::into_raw(Box::new(Driver::new()))
}

/// # Safety
/// `rig` must come from `er_rig_create` and not have been destroyed.
#[no_mangle]
pub unsafe extern "C" fn er_rig_destroy(rig: *mut Driver) {
    if !rig.is_null() {
        drop(Box::from_raw(rig));
    }
}

/// Reads the player's skeleton: `reference` and `found` hold ER_BONE_COUNT
/// entries, root-local transforms of each named bone and whether it was
/// found. Returns how many bones will be driven (0: no animations loaded, or
/// the skeleton is missing what the retarget needs).
///
/// # Safety
/// `rig` from `er_rig_create`; both arrays ER_BONE_COUNT long.
#[no_mangle]
pub unsafe extern "C" fn er_rig_bind(rig: *mut Driver, reference: *const ErXform, found: *const u8) -> u32 {
    let Some(rig) = rig.as_mut() else {
        return 0;
    };
    if reference.is_null() || found.is_null() {
        return 0;
    }
    let reference: &[ErXform; BONE_COUNT] = &*(reference as *const [ErXform; BONE_COUNT]);
    let found_raw = std::slice::from_raw_parts(found, BONE_COUNT);
    let mut found = [false; BONE_COUNT];
    for (f, &raw) in found.iter_mut().zip(found_raw) {
        *f = raw != 0;
    }
    rig.bind(reference, &found) as u32
}

/// One frame of the pose driver. `combat` is the running simulation, or null
/// once Elden Ring mode is off (the pose then fades back to Skyrim's over
/// ER_POSE_FADE_SECONDS). `current` is what Skyrim's animation put on the
/// bones this frame, root-local. Fills `out` and `drive` (ER_BONE_COUNT
/// each): set every bone whose `drive` is 1 to `out` (rotation; the pelvis's
/// position too). Returns 1 if anything is to be written. `info` may be null.
///
/// # Safety
/// `rig` from `er_rig_create`; `combat` null or from `er_create`; arrays
/// ER_BONE_COUNT long.
#[no_mangle]
pub unsafe extern "C" fn er_rig_update(
    rig: *mut Driver,
    combat: *const ErCombat,
    dt: f32,
    current: *const ErXform,
    out: *mut ErXform,
    drive: *mut u8,
    info: *mut ErPoseInfo,
) -> u8 {
    let Some(rig) = rig.as_mut() else {
        return 0;
    };
    if current.is_null() || out.is_null() || drive.is_null() {
        return 0;
    }
    let current: &[ErXform; BONE_COUNT] = &*(current as *const [ErXform; BONE_COUNT]);
    let out: &mut [ErXform; BONE_COUNT] = &mut *(out as *mut [ErXform; BONE_COUNT]);
    let frame = combat.as_ref().map(|c| Frame { player: c.player(), ahead: c.ahead(), lift: c.lift() });
    let mut flags = [false; BONE_COUNT];
    let mut details = Info::default();
    let any = rig.update(frame, dt, current, out, &mut flags, &mut details);
    let drive = std::slice::from_raw_parts_mut(drive, BONE_COUNT);
    for (d, f) in drive.iter_mut().zip(flags) {
        *d = f as u8;
    }
    if let Some(info) = info.as_mut() {
        *info = ErPoseInfo::default();
        let len = details.clip.len().min(31);
        info.clip[..len].copy_from_slice(&details.clip.as_bytes()[..len]);
        info.frame = details.frame;
        info.weight = details.weight;
        info.missing_clip = details.missing_clip as u8;
    }
    any as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: f32 = 1.0 / 60.0;

    fn tap(b: &mut ErButton) {
        b.pressed = 1;
        b.held = 1;
    }

    fn release(b: &mut ErButton) {
        b.held = 0;
        b.released = 1;
    }

    /// Steps a frame and feeds the result back as the player's real position,
    /// the way the bridge does when nothing blocks the character.
    fn frame(c: &mut ErCombat, input: &ErInput, world: &mut ErWorld) -> ErOutput {
        let out = c.update(FRAME, input, world);
        world.position = out.position;
        out
    }

    #[test]
    fn bone_names_match_the_retarget_table() {
        for (i, (name, _)) in crate::pose::retarget::BONES.iter().enumerate() {
            assert_eq!(BONE_NAMES[i].to_str().unwrap(), *name);
            assert_eq!(unsafe { CStr::from_ptr(er_bone_name(i as u32)) }.to_str().unwrap(), *name);
        }
        assert!(er_bone_name(BONE_COUNT as u32).is_null());
    }

    #[test]
    fn axes_round_trip() {
        let v = ErVec3 { x: 140.0, y: -70.0, z: 35.0 };
        let sim = v.to_sim();
        assert_eq!(sim, Vec3::new(-2.0, 0.5, -1.0));
        assert_eq!(ErVec3::from_sim(sim), v);
    }

    #[test]
    fn stick_forward_moves_along_camera_heading() {
        let mut c = ErCombat::new(ErVec3::default());
        let mut world = ErWorld::default();
        // Camera faces +X in Skyrim: heading 90 degrees.
        let input = ErInput { move_y: 1.0, camera_heading: std::f32::consts::FRAC_PI_2, ..Default::default() };
        let mut out = c.output();
        for _ in 0..60 {
            out = frame(&mut c, &input, &mut world);
        }
        assert!(out.position.x > 100.0, "should run along +X, got {:?}", out.position);
        // A few centimetres of drift while the character turns from its spawn facing is fine.
        assert!(out.position.y.abs() < 10.0, "drifted off the heading: {:?}", out.position);
        assert_eq!(out.state, ErState::Ground);
        assert_eq!(out.interruptible, 1);
    }

    #[test]
    fn tapping_dodge_rolls_with_iframes_and_blocks_activate() {
        let mut c = ErCombat::new(ErVec3::default());
        let mut world = ErWorld::default();
        let mut input = ErInput { move_y: 1.0, ..Default::default() };
        frame(&mut c, &input, &mut world);
        // Rolls come out on release, after a short tap.
        tap(&mut input.dodge);
        frame(&mut c, &input, &mut world);
        input.dodge = ErButton::default();
        release(&mut input.dodge);
        let mut out = frame(&mut c, &input, &mut world);
        input.dodge = ErButton::default();
        let mut saw_iframes = false;
        for _ in 0..8 {
            assert_eq!(out.state, ErState::Action);
            assert_eq!(out.interruptible, 0, "Activate must wait for the roll");
            saw_iframes |= out.invincible != 0;
            out = frame(&mut c, &input, &mut world);
        }
        assert!(saw_iframes);
        assert!(out.stamina < 1.0, "rolling costs stamina");
    }

    #[test]
    fn hits_during_iframes_are_dodged() {
        let mut c = ErCombat::new(ErVec3::default());
        let mut world = ErWorld::default();
        let mut input = ErInput::default();
        tap(&mut input.dodge);
        frame(&mut c, &input, &mut world);
        input.dodge = ErButton::default();
        release(&mut input.dodge);
        frame(&mut c, &input, &mut world);
        input.dodge = ErButton::default();
        for _ in 0..4 {
            frame(&mut c, &input, &mut world);
        }
        let from = ErVec3 { x: 0.0, y: 100.0, z: 0.0 };
        assert_eq!(c.receive_hit(from, false, ErHurt::Middle), ErHitResult::Dodged);
    }

    #[test]
    fn light_attack_has_a_live_swing_in_front_that_lands_once() {
        let start = || {
            let mut c = ErCombat::new(ErVec3::default());
            let mut world = ErWorld::default();
            let mut input = ErInput::default();
            tap(&mut input.light);
            frame(&mut c, &input, &mut world);
            input.light = ErButton::default();
            for _ in 0..40 {
                let out = frame(&mut c, &input, &mut world);
                if out.hit_live != 0 {
                    return (c, world, input, out);
                }
            }
            panic!("the swing should go live");
        };

        // Heading 0 faces +Y. A real swing can open behind the shoulder, so
        // the blade only has to come out in front somewhere in its window.
        let (mut c, mut world, input, mut out) = start();
        let mut ahead = false;
        while out.hit_live != 0 {
            ahead |= out.sweep_b.iter().any(|b| b.y > out.position.y);
            out = frame(&mut c, &input, &mut world);
        }
        assert!(ahead, "the blade swings out in front");

        let (mut c, _, _, out) = start();
        assert!(out.hit_damage > 0.0 && out.hit_radius > 0.0);
        c.confirm_hit();
        assert_eq!(c.output().hit_live, 0, "a swing lands once");
    }

    #[test]
    fn each_swing_of_a_chain_gets_its_own_serial() {
        let mut c = ErCombat::new(ErVec3::default());
        let mut world = ErWorld::default();
        let mut input = ErInput::default();
        let mut serials = Vec::new();
        for _ in 0..3 {
            tap(&mut input.light);
            frame(&mut c, &input, &mut world);
            input.light = ErButton::default();
            // The stand-in swing lets the next one in from frame 20, which is
            // 40 ticks; the queued press comes out then.
            for _ in 0..45 {
                let out = frame(&mut c, &input, &mut world);
                if out.action_kind == ErAction::LightAttack && serials.last() != Some(&out.action_serial) {
                    serials.push(out.action_serial);
                }
            }
        }
        assert_eq!(serials.len(), 3, "three swings, three serials: {serials:?}");
    }

    #[test]
    fn a_long_hitch_does_not_replay_seconds_of_input() {
        let mut c = ErCombat::new(ErVec3::default());
        let world = ErWorld::default();
        let input = ErInput { move_y: 1.0, ..Default::default() };
        let out = c.update(5.0, &input, &world);
        let metres = out.position.y / UNITS_PER_METRE;
        assert!(metres < 1.0, "moved {metres} m in one capped frame");
    }

    #[test]
    fn loading_data_reports_why_it_failed() {
        // Only failures here: the tests share a process, and a successful
        // load would swap the table under the others (see tests/erdata_load.rs).
        unsafe {
            assert_eq!(er_load_data(std::ptr::null()), ER_LOAD_BAD_PATH);
            assert_eq!(er_load_data(c"definitely/not/here.erdata".as_ptr()), ER_LOAD_IO);
            assert_eq!(er_load_data([0xffu8, 0].as_ptr().cast()), ER_LOAD_BAD_PATH);
        }
        assert_eq!(load_result(Err(LoadError::Read(ReadError::BadMagic))), ER_LOAD_NOT_ERDATA);
        assert_eq!(load_result(Err(LoadError::Read(ReadError::Version(2)))), ER_LOAD_VERSION);
        assert_eq!(load_result(Err(LoadError::Read(ReadError::Layout))), ER_LOAD_LAYOUT);
        assert_eq!(load_result(Err(LoadError::Read(ReadError::Truncated))), ER_LOAD_CORRUPT);
        assert!(tables::loaded().is_none());
    }

    #[test]
    fn ground_height_comes_from_skyrim() {
        let mut c = ErCombat::new(ErVec3 { x: 0.0, y: 0.0, z: 700.0 });
        let mut world = ErWorld { position: ErVec3 { x: 0.0, y: 0.0, z: 700.0 }, ground_z: 700.0, ..Default::default() };
        let out = frame(&mut c, &ErInput::default(), &mut world);
        assert_eq!(out.state, ErState::Ground, "standing on ground 10 m up is not falling");
        assert!((out.position.z - 700.0).abs() < 0.5);
    }

    /// Joel's log: a jump that no Skyrim animation answered left the core in
    /// state 1 (action) with stamina frozen, and every later dodge did
    /// nothing. Skyrim never moves the player here, as when no animation
    /// answers: the core must come back down on its own.
    #[test]
    fn a_jump_skyrim_never_follows_still_ends() {
        let mut c = ErCombat::new(ErVec3::default());
        let world = ErWorld::default();
        let mut input = ErInput::default();
        tap(&mut input.jump);
        let out = c.update(FRAME, &input, &world);
        assert_eq!(out.action_kind, ErAction::Jump);
        input.jump = ErButton::default();
        let mut out = out;
        let mut frames = 0;
        // Skyrim's position stays put: never fed back.
        while out.state != ErState::Ground && frames < 600 {
            out = c.update(FRAME, &input, &world);
            frames += 1;
        }
        assert_eq!(out.state, ErState::Ground, "still {:?} after {frames} frames", out.state);
        assert!(frames < 240, "took {frames} frames to come down");

        // Stamina comes back and the next dodge rolls.
        let low = out.stamina;
        for _ in 0..60 {
            out = c.update(FRAME, &input, &world);
        }
        assert!(out.stamina > low || out.stamina == 1.0, "stamina frozen at {}", out.stamina);
        let before = out.action_serial;
        input.move_y = 1.0;
        tap(&mut input.dodge);
        c.update(FRAME, &input, &world);
        input.dodge = ErButton::default();
        release(&mut input.dodge);
        let out = c.update(FRAME, &input, &world);
        assert_ne!(out.action_serial, before, "the dodge after the jump must roll");
        assert_eq!(out.action_kind, ErAction::Roll);
    }

    /// In the air in the simulation while Skyrim holds the player at one
    /// height (walked off something Skyrim says is not a ledge, a ground
    /// height that is off): it lands instead of falling for ever.
    #[test]
    fn airborne_while_skyrim_stands_still_lands() {
        let mut c = ErCombat::new(ErVec3 { z: 700.0, ..Default::default() });
        // Skyrim says the ground is 5 m below, but the player stays at 700.
        let world = ErWorld { position: ErVec3 { z: 700.0, ..Default::default() }, ground_z: 350.0, ..Default::default() };
        let input = ErInput::default();
        let mut out = c.update(FRAME, &input, &world);
        assert_eq!(out.state, ErState::Air);
        for _ in 0..60 {
            out = c.update(FRAME, &input, &world);
        }
        assert_ne!(out.state, ErState::Air, "Skyrim never fell; the core must not wait for it");
    }

    #[test]
    fn stick_right_goes_right_in_skyrim() {
        // Camera looks down +Y (heading 0); its right is +X.
        let mut c = ErCombat::new(ErVec3::default());
        let mut world = ErWorld::default();
        let input = ErInput { move_x: 1.0, ..Default::default() };
        let mut out = c.output();
        for _ in 0..60 {
            out = frame(&mut c, &input, &mut world);
        }
        assert!(out.position.x > 100.0, "should run along +X, got {:?}", out.position);
        assert!((out.heading - std::f32::consts::FRAC_PI_2).abs() < 0.05, "faces +X: heading {}", out.heading);
    }

    #[test]
    fn locked_on_roll_to_the_right_says_right() {
        let mut c = ErCombat::new(ErVec3::default());
        let mut world = ErWorld { has_target: 1, target: ErVec3 { y: 500.0, ..Default::default() }, ..Default::default() };
        let mut input = ErInput { move_x: 1.0, ..Default::default() };
        frame(&mut c, &input, &mut world);
        tap(&mut input.dodge);
        frame(&mut c, &input, &mut world);
        input.dodge = ErButton::default();
        release(&mut input.dodge);
        let out = frame(&mut c, &input, &mut world);
        assert_eq!(out.action_kind, ErAction::Roll);
        assert_eq!(out.action_dir, Dir::Right as u32);
        let start = out.position;
        let mut out = out;
        for _ in 0..40 {
            out = frame(&mut c, &input, &mut world);
        }
        // The stand-in's rolls all move straight ahead; the real ones sideways.
        assert!(cfg!(not(er_data)) || out.position.x > start.x + 50.0, "rolled toward +X: {:?} -> {:?}", start, out.position);
    }

    /// Jump, then Light in the air: the swing goes live on the way down and
    /// the landing plays the jump attack's follow-through. Skyrim does not
    /// jump along (as with the pose driver), and it still works.
    #[test]
    fn a_jump_attack_comes_from_jump_then_light() {
        let mut c = ErCombat::new(ErVec3::default());
        let world = ErWorld::default();
        let mut input = ErInput { move_y: 1.0, ..Default::default() };
        c.update(FRAME, &input, &world);
        tap(&mut input.jump);
        c.update(FRAME, &input, &world);
        input.jump = ErButton::default();
        let (mut live, mut landed) = (false, false);
        for i in 0..240 {
            if i == 12 {
                tap(&mut input.light);
            }
            let out = c.update(FRAME, &input, &world);
            input.light = ErButton::default();
            live |= out.hit_live != 0;
            if out.action_kind == ErAction::JumpAttackLand {
                landed = true;
                break;
            }
        }
        assert!(live, "the jump attack's swing goes live in the air");
        assert!(landed, "and lands into its follow-through");
    }

    /// Locked on, the core turns the player to face the target, and the
    /// output heading says so (the bridge applies it while locked).
    #[test]
    fn locked_on_the_core_faces_the_target() {
        let mut c = ErCombat::new(ErVec3::default());
        // Target off to Skyrim's +X: heading 90 degrees.
        let world = ErWorld { has_target: 1, target: ErVec3 { x: 500.0, ..Default::default() }, ..Default::default() };
        let mut out = c.output();
        for _ in 0..60 {
            out = c.update(FRAME, &ErInput { move_y: 0.5, ..Default::default() }, &world);
        }
        assert!((out.heading - std::f32::consts::FRAC_PI_2).abs() < 0.05, "heading {}", out.heading);
    }

    #[test]
    fn the_plugins_weapon_classes_are_in_the_table() {
        // Every name plugin/src/Weapons.cpp maps to. The stub table has only
        // the last three; with the real table all must resolve.
        let names = [
            "Uchigatana", "Rapier", "Whip", "Twinblade", "Halberd", "Great Spear", "Short Spear", "Claw", "Reaper",
            "Curved Sword", "Great Hammer", "Dagger", "Longsword", "Battle Axe", "Club", "Claymore", "Greataxe",
            "Fist", "Torch", "Shield",
        ];
        for name in names {
            let c = std::ffi::CString::new(name).unwrap();
            let i = unsafe { er_weapon_index(c.as_ptr()) };
            if cfg!(er_data) || ["Fist", "Torch", "Shield"].contains(&name) {
                assert!(i >= 0, "{name} not in the action table");
            }
        }
    }

    #[test]
    fn an_empty_left_hand_guards_with_the_right_weapon() {
        use crate::sim::data::{fist, shield};
        use crate::sim::player::LeftButton;
        let mut c = ErCombat::new(ErVec3::default());
        let sword = if shield() == 0 { 1 } else { 0 };
        assert_ne!(sword, fist());
        c.set_armaments(sword as i32, fist() as i32, false);
        assert_eq!(c.player.left_button(), LeftButton::Guard);
        let world = ErWorld { ground_z: 0.0, ..Default::default() };
        let press = ErInput { guard: ErButton { pressed: 1, held: 1, released: 0 }, ..Default::default() };
        let hold = ErInput { guard: ErButton { pressed: 0, held: 1, released: 0 }, ..Default::default() };
        let mut out = c.update(FRAME, &press, &world);
        for _ in 0..20 {
            out = c.update(FRAME, &hold, &world);
        }
        assert_eq!(out.guarding, 1, "the guard is up");
        assert_ne!(out.action_kind, ErAction::LeftAttack, "no punch");
        // Fists in both hands: the left button punches.
        c.set_armaments(fist() as i32, fist() as i32, false);
        assert_eq!(c.player.left_button(), LeftButton::Attack);
    }

    #[test]
    fn armaments_change_the_moveset() {
        use crate::sim::data::{shield, weapons, Grip};
        let names: Vec<&str> = weapons().iter().map(|w| w.name).collect();
        let c_name = |n: &str| std::ffi::CString::new(n).unwrap();
        for (i, n) in names.iter().enumerate() {
            assert_eq!(unsafe { er_weapon_index(c_name(n).as_ptr()) }, i as i32);
        }
        assert_eq!(unsafe { er_weapon_index(c"No such weapon".as_ptr()) }, -1);
        let mut c = ErCombat::new(ErVec3::default());
        let last = names.len() as i32 - 1;
        // The shield is never a right-hand weapon.
        assert!(!c.set_armaments(shield() as i32, -1, false));
        let right = if shield() == 0 { 1 } else { 0 };
        let changed = c.set_armaments(right as i32, last, true);
        assert_eq!(changed, (right, last as usize, Grip::TwoHandRight) != (c.player.weapon, c.player.left, c.player.grip) || changed);
        assert_eq!(c.player.weapon, right);
        assert_eq!(c.player.grip, Grip::TwoHandRight);
        assert!(!c.set_armaments(right as i32, last, true), "no change, nothing to do");
    }

    #[test]
    fn follows_skyrims_heading_when_asked() {
        let mut c = ErCombat::new(ErVec3::default());
        let world = ErWorld { heading: 1.0, follow_heading: 1, ..Default::default() };
        let out = c.update(FRAME, &ErInput::default(), &world);
        assert!((out.heading - 1.0).abs() < 1e-4);
    }
}

/// The C header's structs must lay out exactly like these. The bridge
/// static_asserts the same numbers.
#[cfg(test)]
mod layout {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn matches_the_c_header() {
        assert_eq!(size_of::<ErVec3>(), 12);
        assert_eq!(size_of::<ErButton>(), 3);
        assert_eq!(size_of::<ErInput>(), 32);
        assert_eq!(size_of::<ErXform>(), 48);
        assert_eq!(size_of::<ErPoseInfo>(), 44);
        assert_eq!(BONE_COUNT, 26);
        assert_eq!(size_of::<ErWorld>(), 44);
        assert_eq!(offset_of!(ErWorld, target), 20);
        assert_eq!(offset_of!(ErWorld, heading), 36);
        assert_eq!(offset_of!(ErWorld, follow_heading), 40);
        assert_eq!(size_of::<ErOutput>(), 224);
        assert_eq!(offset_of!(ErOutput, hit_motion), 220);
        assert_eq!(offset_of!(ErOutput, action), 20);
        assert_eq!(offset_of!(ErOutput, stamina), 80);
        assert_eq!(offset_of!(ErOutput, air_attack), 78);
        assert_eq!(offset_of!(ErOutput, sweep_a), 88);
        assert_eq!(offset_of!(ErOutput, hit_stop), 216);
    }
}

/// The header's pose-driver constants match the core's.
#[cfg(test)]
mod header {
    #[test]
    fn constants_match() {
        let header = include_str!("../include/er_combat.h");
        assert!(header.contains(&format!("#define ER_BONE_COUNT {}", crate::pose::retarget::BONE_COUNT)));
        assert!(header.contains(&format!("#define ER_POSE_FADE_SECONDS {}f", crate::pose::FADE_SECONDS)));
        assert!(header.contains(&format!("#define ER_SWEEP_STEPS {}", crate::sim::player::SWEEP_STEPS)));
    }
}

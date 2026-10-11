//! The C interface the SKSE bridge calls. See `include/er_combat.h`.
//!
//! Everything crossing this boundary is in Skyrim's space: game units
//! (`UNITS_PER_METRE` to the metre), Z up, and heading in radians with 0
//! facing +Y and turning clockwise toward +X. The simulation works in metres,
//! Y up, yaw 0 facing +Z and turning toward +X, so the two yaws agree and only
//! the axes and scale change.

use std::ffi::{c_char, CStr};
use std::path::Path;

use glam::{Vec2, Vec3};

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
    fn to_sim(self) -> Vec3 {
        Vec3::new(self.x, self.z, self.y) / UNITS_PER_METRE
    }

    fn from_sim(v: Vec3) -> Self {
        let v = v * UNITS_PER_METRE;
        Self { x: v.x, y: v.z, z: v.y }
    }
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
}

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
    pub guarding: u8,
    pub invincible: u8,
    pub airborne: u8,
    /// Nothing is playing that pressing Activate would cut short, so the
    /// bridge may let Activate through to Skyrim (doors, loot, talking).
    pub interruptible: u8,
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
}

impl ErCombat {
    pub fn new(position: ErVec3) -> Self {
        Self { player: Player::new(position.to_sim()), level: Level::flat(), pending: 0.0, serial: 0, last_action: None }
    }

    pub fn update(&mut self, dt: f32, input: &ErInput, world: &ErWorld) -> ErOutput {
        // Skyrim's collision has the last word on where the player is.
        self.player.pos = world.position.to_sim();
        self.level.ground = Some(world.ground_z / UNITS_PER_METRE);
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
            heading: p.yaw,
            state,
            action: name,
            action_frame: frame,
            action_kind: kind,
            action_dir: dir as u32,
            action_serial: self.serial,
            speed: p.speed * UNITS_PER_METRE,
            sprinting: p.sprinting as u8,
            crouching: p.crouching as u8,
            guarding: p.guard_up() as u8,
            invincible: p.invincible() as u8,
            airborne: p.airborne() as u8,
            interruptible: (interruptible && p.swap.is_none()) as u8,
            stamina: (p.stamina / crate::sim::data::max_stamina()).clamp(0.0, 1.0),
            hit_live: hit.is_some() as u8,
            sweep_a,
            sweep_b,
            hit_radius: hit.map_or(0.0, |h| h.radius * UNITS_PER_METRE),
            hit_damage: hit.map_or(0.0, |h| h.attack * h.mv),
            hit_stop: hit.map_or(0.0, |h| h.stop * crate::sim::data::HIT_STOP_SCALE),
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
        cam_yaw: i.camera_heading,
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
    fn axes_round_trip() {
        let v = ErVec3 { x: 140.0, y: -70.0, z: 35.0 };
        let sim = v.to_sim();
        assert_eq!(sim, Vec3::new(2.0, 0.5, -1.0));
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
        let mut c = ErCombat::new(ErVec3::default());
        let mut world = ErWorld::default();
        let mut input = ErInput::default();
        tap(&mut input.light);
        frame(&mut c, &input, &mut world);
        input.light = ErButton::default();
        let mut live = None;
        for _ in 0..40 {
            let out = frame(&mut c, &input, &mut world);
            if out.hit_live != 0 {
                live = Some(out);
                break;
            }
        }
        let out = live.expect("the swing should go live");
        // Heading 0 faces +Y: the blade is out in front.
        assert!(out.sweep_b.iter().all(|b| b.y > out.position.y));
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
        assert_eq!(size_of::<ErWorld>(), 36);
        assert_eq!(offset_of!(ErWorld, target), 20);
        assert_eq!(size_of::<ErOutput>(), 220);
        assert_eq!(offset_of!(ErOutput, action), 20);
        assert_eq!(offset_of!(ErOutput, stamina), 80);
        assert_eq!(offset_of!(ErOutput, sweep_a), 88);
        assert_eq!(offset_of!(ErOutput, hit_stop), 216);
    }
}

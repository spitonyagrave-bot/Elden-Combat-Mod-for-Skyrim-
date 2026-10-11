//! Which Elden Ring clip plays for the simulation's state, sampled and
//! cross-faded into one pose per frame. A port of the sandbox's
//! `rig::playing` and `rig::animate` from upstream (Funny-Bones,
//! ELDEN-RING-Combat-Rewrite, MIT), without its drawing and foot placement:
//! Skyrim's own ground and character controller take care of the feet.

use glam::Vec3;

use super::clips::{set_vec3, vec3, Clips};
use crate::sim::data::*;
use crate::sim::player::{Player, State};

// Animation ids within a category. Stances (how a weapon is carried) each have
// their own idle and guard; two-handed stances also have their own locomotion.
const IDLE: u32 = 0;
const GUARD: u32 = 100;
const WALK: u32 = 20000;
const RUN: u32 = 20100;
const SPRINT: u32 = 20200;
const CROUCH_IDLE: u32 = 300000;
const CROUCH_WALK: u32 = 320000;
const CROUCH_RUN: u32 = 320100;
const RUN_STOP: u32 = 22100;
const CROUCH_RUN_STOP: u32 = 322100;
const CROUCH_ENTER: u32 = 390000;
const CROUCH_EXIT: u32 = 390001;
/// Coming to rest this soon (seconds) after running plays the run's stop.
const STOP_WITHIN: f32 = 0.3;
/// In the air after a jump's arc, and for the body of any fall.
const AIR_LOOP: &str = "a000_202040";
/// Stepping off a ledge, before settling into the loop.
const FALL_START: &str = "a000_004000";
const FALL_START_FRAMES: f32 = 24.0;
const DEATH: &str = "a000_017002";
/// Seconds for an arm layer (guard, one-handed carry) to blend in or out.
const LAYER_BLEND: f32 = 0.12;
/// Shortest cross-fade between clips, in frames, so nothing ever snaps.
const MIN_BLEND_FRAMES: f32 = 4.0;
/// Between two locomotion loops (walk/run/sprint, any direction) the fade
/// is at least this long: the feet change gait instead of popping.
const LOCO_BLEND_FRAMES: f32 = 8.0;
/// A locomotion loop plays at least this long before another takes over,
/// so a stick or speed right on a threshold doesn't flip clips every frame.
const LOCO_HOLD_SECONDS: f32 = 0.15;

const LEFT_ARM: [&str; 12] = [
    "L_Clavicle", "L_UpperArm", "L_Forearm", "L_Hand", "L_Weapon",
    "L_Finger0", "L_Finger01", "L_Finger02", "L_Finger1", "L_Finger2", "L_Finger21", "L_Finger22",
];
const RIGHT_ARM: [&str; 12] = [
    "R_Clavicle", "R_UpperArm", "R_Forearm", "R_Hand", "R_Weapon",
    "R_Finger0", "R_Finger01", "R_Finger02", "R_Finger1", "R_Finger2", "R_Finger21", "R_Finger22",
];

pub fn clip_name(category: u8, id: u32) -> String {
    format!("a{category:03}_{id:06}")
}

/// Stance category for how the player is currently holding things.
fn stance(p: &Player) -> u8 {
    let weapons = weapons();
    let of = |i: usize, two: usize| weapons.get(i).map_or(0, |w| w.stance[two]);
    match p.grip {
        Grip::OneHand => of(p.weapon, 0),
        Grip::TwoHandRight => of(p.weapon, 1),
        Grip::TwoHandLeft => of(p.left, 1),
    }
}

/// The pose being shown and everything needed to move it on smoothly.
#[derive(Default)]
pub struct Rig {
    /// Clip currently playing, and the pose being cross-faded out of.
    pub clip: String,
    pub frame: f32,
    from: Vec<f32>,
    fade: f32,
    fade_len: f32,
    shown: Vec<f32>,
    /// Stride phase, 0..1, shared by every locomotion loop.
    phase: f32,
    guard: f32,
    carry: f32,
    swap: f32,
    rest: Option<(String, f32)>,
    /// The locomotion loop last chosen (id, native speed) and for how long.
    loco: Option<(u32, f32)>,
    loco_held: f32,
    clip_looped: bool,
    ran: f32,
    ran_dir: u32,
    still: bool,
    crouched: bool,
    time: f32,
    target: Vec<f32>,
    layer: Vec<f32>,
}

impl Rig {
    pub fn new() -> Self {
        Self { ran: f32::MAX, still: true, ..Default::default() }
    }

    /// Which clip the simulation's state corresponds to: (clip, frame, loops).
    fn playing(&mut self, p: &Player, clips: &Clips, dt: f32, ahead: f32) -> (String, f32, bool) {
        if let Some(attack) = p.air_attack {
            return (attack.def.source.to_string(), attack.f + ahead, false);
        }
        let stance = stance(p);
        let grounded = matches!(p.state, State::Ground);
        let still = grounded && p.speed < 0.05;
        let running_from = if p.crouching {
            (crouch_walk_speed() + crouch_run_speed()) / 2.0
        } else {
            (walk_speed() + run_speed()) / 2.0
        };
        if grounded && p.speed > running_from {
            self.ran = 0.0;
        } else {
            self.ran += dt;
        }
        if !still {
            self.rest = None;
        } else if p.crouching != self.crouched {
            self.rest = Some((clip_name(0, if p.crouching { CROUCH_ENTER } else { CROUCH_EXIT }), 0.0));
        } else if !self.still && self.ran < STOP_WITHIN {
            let options = if p.crouching {
                [clip_name(0, CROUCH_RUN_STOP), clip_name(0, CROUCH_RUN_STOP)]
            } else {
                let own = if p.grip == Grip::OneHand { 0 } else { stance };
                [clip_name(own, RUN_STOP + self.ran_dir), clip_name(own, RUN_STOP)]
            };
            self.rest = options.into_iter().find(|name| clips.get(name).is_some()).map(|name| (name, 0.0));
        }
        self.still = still;
        self.crouched = p.crouching;
        if let Some((name, frame)) = &mut self.rest {
            *frame += ANIM_FPS * dt;
            match clips.get(name) {
                Some(clip) if *frame < (clip.frames - 1) as f32 => return (name.clone(), *frame, false),
                _ => self.rest = None,
            }
        }
        let time = self.time;
        if !(matches!(p.state, State::Ground) && p.speed >= 0.05) {
            self.loco = None;
        }
        match p.state {
            State::Dead { t } => (DEATH.to_string(), t + ahead, false),
            State::Air(a) if !a.jumped && a.f < FALL_START_FRAMES => (FALL_START.to_string(), a.f + ahead, false),
            State::Air(_) => (AIR_LOOP.to_string(), time * ANIM_FPS, true),
            State::Act(a) => (a.id.def().source.to_string(), a.f + ahead, false),
            State::Ground => {
                if p.speed < 0.05 {
                    return if p.crouching {
                        (clip_name(0, CROUCH_IDLE), time * ANIM_FPS, true)
                    } else {
                        (clip_name(stance, IDLE), time * ANIM_FPS, true)
                    };
                }
                let facing = p.facing();
                let along = p.move_dir.dot(facing);
                let across = p.move_dir.dot(Vec3::new(facing.z, 0.0, -facing.x));
                // Forward, back, left, right.
                let dir = if along.abs() >= across.abs() { (along < 0.0) as u32 } else { 2 + (across < 0.0) as u32 };
                self.ran_dir = dir;
                let (mut id, mut native) = if p.crouching && p.speed > (crouch_walk_speed() + crouch_run_speed()) / 2.0 {
                    (CROUCH_RUN + dir, crouch_run_speed())
                } else if p.crouching {
                    (CROUCH_WALK + dir, crouch_walk_speed())
                } else if p.sprinting && p.speed > run_speed() {
                    (SPRINT, sprint_speed())
                } else if p.speed > (walk_speed() + run_speed()) / 2.0 {
                    (RUN + dir, [run_speed(), run_back_speed(), run_side_speed(), run_side_speed()][dir as usize])
                } else {
                    (WALK + dir, walk_speed())
                };
                self.loco_held += dt;
                match self.loco {
                    Some((held, held_native)) if held != id && self.loco_held < LOCO_HOLD_SECONDS => {
                        id = held;
                        native = held_native;
                    }
                    Some((held, _)) if held == id => {}
                    _ => {
                        self.loco = Some((id, native));
                        self.loco_held = 0.0;
                    }
                }
                let mut clip = clip_name(stance, id);
                if p.grip == Grip::OneHand || p.crouching || clips.get(&clip).is_none() {
                    clip = clip_name(0, id);
                }
                // Play the loop at whatever rate keeps the feet matched to the ground speed.
                let last = clips.get(&clip).map_or(1.0, |c| (c.frames - 1).max(1) as f32);
                self.phase = (self.phase + p.speed / native.max(0.1) * ANIM_FPS * dt / last).fract();
                (clip, self.phase * last, true)
            }
        }
    }

    /// Advances by `dt` seconds and returns the pose to show, in the baked
    /// model space. `ahead` is how many animation frames the simulation's
    /// clock has run past its last tick (the part of a tick not simulated
    /// yet), so the pose moves on every frame, not every tick. None if the
    /// clip the state calls for is not in the file.
    pub fn update(&mut self, p: &Player, clips: &Clips, dt: f32, ahead: f32) -> Option<&[f32]> {
        let dt = dt.clamp(0.0, 0.25);
        self.time += dt;
        let ahead = if p.hit_stop > 0.0 { 0.0 } else { ahead };
        let (name, frame, looped) = self.playing(p, clips, dt, ahead);
        let clip = clips.get(&name)?;
        let mut target = std::mem::take(&mut self.target);
        clips.sample(clip, frame, looped, &mut target);
        self.frame = frame;

        // Cross-fade from whatever was shown, over the clip's authored blend time.
        if name != self.clip {
            let was_loco = self.clip_looped && self.loco.is_some();
            self.clip = name;
            self.from.clone_from(&self.shown);
            self.fade = 0.0;
            let min = if looped && was_loco { LOCO_BLEND_FRAMES } else { MIN_BLEND_FRAMES };
            self.fade_len = if self.from.len() != target.len() { 0.0 } else { clip.blend.max(min) / ANIM_FPS };
        }
        self.clip_looped = looped;
        self.fade += dt;
        let t = if self.fade_len > 0.0 { (self.fade / self.fade_len).clamp(0.0, 1.0) } else { 1.0 };
        let t = t * t * (3.0 - 2.0 * t);
        self.shown.clear();
        if t >= 1.0 {
            self.shown.extend_from_slice(&target);
        } else {
            self.shown.extend(self.from.iter().zip(target.iter()).map(|(a, b)| a + (b - a) * t));
        }

        let two_handed = p.grip != Grip::OneHand;
        let stance = stance(p);
        let grounded = matches!(p.state, State::Ground);
        let raising = p.guarding || matches!(p.state, State::Act(a) if a.id == ActionId::GuardHit);
        let step = dt / LAYER_BLEND;
        self.guard = (self.guard + if raising { step } else { -step }).clamp(0.0, 1.0);
        let stopping = self.rest.is_some() && !p.crouching && !self.clip.ends_with(&format!("{CROUCH_EXIT:06}"));
        let carrying = !two_handed && stance != 0 && grounded && (p.speed >= 0.05 || stopping);
        self.carry = (self.carry + if carrying { step } else { -step }).clamp(0.0, 1.0);

        target.clone_from(&self.shown);
        let time = self.time;
        let mut layer = std::mem::take(&mut self.layer);
        // An arm layer takes those joints from another clip, re-seated on the
        // chest of whatever the body is doing.
        let mut overlay = |pose: &mut Vec<f32>, name: &str, at: Option<f32>, arms: &[&[&str]], weight: f32| {
            let Some(clip) = clips.get(name) else {
                return;
            };
            if weight <= 0.0 {
                return;
            }
            match at {
                Some(frame) => clips.sample(clip, frame, false, &mut layer),
                None => clips.sample(clip, time * ANIM_FPS, true, &mut layer),
            }
            let chest = clips.joint("Spine2");
            let offset = vec3(pose, chest) - vec3(&layer, chest);
            for joint in arms.iter().flat_map(|arm| arm.iter()) {
                let Some(at) = clips.joint_index(joint) else {
                    continue;
                };
                let blended = vec3(pose, at).lerp(vec3(&layer, at) + offset, weight);
                set_vec3(pose, at, blended);
                if let (true, Some(at)) = (joint.ends_with("Weapon"), clips.axes_index(joint)) {
                    for i in at..at + 9 {
                        pose[i] += (layer[i] - pose[i]) * weight;
                    }
                }
            }
        };
        overlay(&mut target, &clip_name(stance, IDLE), None, &[&RIGHT_ARM], self.carry);
        let (guard_stance, guard_arms): (u8, &[&[&str]]) = if two_handed {
            (stance, &[&LEFT_ARM, &RIGHT_ARM])
        } else {
            (weapons().get(shield()).map_or(0, |w| w.stance[0]), &[&LEFT_ARM])
        };
        overlay(&mut target, &clip_name(guard_stance, GUARD), None, guard_arms, self.guard);
        match p.swap {
            Some(swap) => {
                let def = swap.kind.def();
                let frame = swap.f + ahead;
                self.swap = (self.swap + step).min(1.0);
                let (clip, at, weight) = if frame < def.start_len {
                    (def.start, frame, self.swap)
                } else {
                    let t = ((frame - def.start_len) / def.end_len.max(1e-3)).clamp(0.0, 1.0);
                    (def.end, frame - def.start_len, self.swap * (1.0 - t * t * (3.0 - 2.0 * t)))
                };
                overlay(&mut target, clip, Some(at), &[&LEFT_ARM, &RIGHT_ARM], weight);
            }
            None => self.swap = 0.0,
        }
        self.layer = layer;
        self.target = target;
        Some(&self.target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pose::clips::Clips;
    use crate::pose::tests::{clips_file, row, standing};
    use glam::Vec3;

    #[test]
    fn a_speed_on_the_walk_run_threshold_does_not_flip_clips_every_frame() {
        let stand = row(&standing());
        let names: Vec<String> = [IDLE, WALK, RUN].iter().map(|&id| clip_name(0, id)).collect();
        let list: Vec<(&str, f32, Vec<Vec<f32>>)> = names.iter().map(|n| (n.as_str(), 0.0, vec![stand.clone(); 30])).collect();
        let clips = Clips::read(&clips_file(&list)).unwrap();
        let mut rig = Rig::new();
        let mut p = Player::new(Vec3::ZERO);
        p.move_dir = p.facing();
        let threshold = (walk_speed() + run_speed()) / 2.0;
        let dt = 1.0 / 60.0;
        let mut changes = 0;
        let mut last = String::new();
        for i in 0..60 {
            // Just under, then just over, every frame.
            p.speed = threshold + if i % 2 == 0 { -0.01 } else { 0.01 };
            rig.update(&p, &clips, dt, 0.0);
            if rig.clip != last {
                changes += 1;
                last = rig.clip.clone();
            }
        }
        // One second of flip-flopping input: a handful of changes, not 60.
        assert!(changes <= 8, "{changes} clip changes in a second");
    }
}

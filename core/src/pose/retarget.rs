//! Puts an Elden Ring pose on Skyrim's skeleton.
//!
//! Nothing about Skyrim's skeleton is assumed beyond its node names: the
//! bridge reads the player's own skeleton once (a reference pose, whatever it
//! is doing at that moment) and the bones' lengths, rest twists and the body's
//! proportions all come from it.
//!
//! Every bone gets a frame built the same way on both skeletons: its first
//! axis points from the joint to the next one down the chain (where the bone
//! points: the swing), its second comes from a nearby landmark that says how
//! the bone is turned about itself (the twist): the line across the hips or
//! shoulders for the trunk, where the forearm or the foot points for a bent
//! limb, the line across the knuckles for the forearm and hand, the face for
//! the head. In the reference pose, the frame built from Skyrim's joints and
//! the bone's real rotation give a fixed offset per bone. Each frame the same
//! frame is built from the Elden Ring pose and the offset put back on: the
//! Skyrim bone then points and turns the way the Elden Ring one does, with
//! Skyrim's own lengths, so nothing stretches.
//!
//! Space: the player's 3D root ("root-local"), Skyrim units, +X right, +Y
//! forward, +Z up. Elden Ring's baked space (+X left, +Y up, +Z forward, metres)
//! maps onto it as (-x, z, y), a proper rotation.

use glam::{Mat3, Quat, Vec3};

use super::clips::{vec3, Clips};

/// A bone's transform in root-local space. `rot` is row-major, the way
/// NiMatrix3 stores it (`rot[r * 3 + c]` is `entry[r][c]`), and maps the
/// bone's local axes into root-local space: column c is local axis c.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ErXform {
    pub rot: [f32; 9],
    pub pos: [f32; 3],
}

impl Default for ErXform {
    fn default() -> Self {
        Self { rot: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0], pos: [0.0; 3] }
    }
}

impl ErXform {
    pub fn mat(&self) -> Mat3 {
        Mat3::from_cols_array(&self.rot).transpose()
    }

    pub fn pos(&self) -> Vec3 {
        Vec3::from_array(self.pos)
    }

    pub fn new(rot: Mat3, pos: Vec3) -> Self {
        Self { rot: rot.transpose().to_cols_array(), pos: pos.to_array() }
    }
}

/// The bones the core knows, in parent-before-child order, with Skyrim's
/// node name (vanilla and XPMSE skeletons alike) and the baked Elden Ring
/// joint that matches it.
pub const BONES: &[(&str, &str)] = &[
    ("NPC Pelvis [Pelv]", "Pelvis"),
    ("NPC Spine [Spn0]", "Spine"),
    ("NPC Spine1 [Spn1]", "Spine1"),
    ("NPC Spine2 [Spn2]", "Spine2"),
    ("NPC Neck [Neck]", "Neck"),
    ("NPC Head [Head]", "Head"),
    ("NPC L Clavicle [LClv]", "L_Clavicle"),
    ("NPC L UpperArm [LUar]", "L_UpperArm"),
    ("NPC L Forearm [LLar]", "L_Forearm"),
    ("NPC L Hand [LHnd]", "L_Hand"),
    ("NPC R Clavicle [RClv]", "R_Clavicle"),
    ("NPC R UpperArm [RUar]", "R_UpperArm"),
    ("NPC R Forearm [RLar]", "R_Forearm"),
    ("NPC R Hand [RHnd]", "R_Hand"),
    ("NPC L Thigh [LThg]", "L_Thigh"),
    ("NPC L Calf [LClf]", "L_Calf"),
    ("NPC L Foot [Lft ]", "L_Foot"),
    ("NPC L Toe0 [LToe]", "L_Toe0"),
    ("NPC R Thigh [RThg]", "R_Thigh"),
    ("NPC R Calf [RClf]", "R_Calf"),
    ("NPC R Foot [Rft ]", "R_Foot"),
    ("NPC R Toe0 [RToe]", "R_Toe0"),
    // Landmarks only: the index and middle knuckles, for how the hand turns.
    ("NPC L Finger10 [LF10]", "L_Finger1"),
    ("NPC L Finger20 [LF20]", "L_Finger2"),
    ("NPC R Finger10 [RF10]", "R_Finger1"),
    ("NPC R Finger20 [RF20]", "R_Finger2"),
];
pub const BONE_COUNT: usize = 26;
const _: () = assert!(BONES.len() == BONE_COUNT);

pub mod bone {
    pub const PELVIS: usize = 0;
    pub const SPINE: usize = 1;
    pub const SPINE1: usize = 2;
    pub const SPINE2: usize = 3;
    pub const NECK: usize = 4;
    pub const HEAD: usize = 5;
    pub const L_CLAVICLE: usize = 6;
    pub const L_UPPERARM: usize = 7;
    pub const L_FOREARM: usize = 8;
    pub const L_HAND: usize = 9;
    pub const R_CLAVICLE: usize = 10;
    pub const R_UPPERARM: usize = 11;
    pub const R_FOREARM: usize = 12;
    pub const R_HAND: usize = 13;
    pub const L_THIGH: usize = 14;
    pub const L_CALF: usize = 15;
    pub const L_FOOT: usize = 16;
    pub const L_TOE: usize = 17;
    pub const R_THIGH: usize = 18;
    pub const R_CALF: usize = 19;
    pub const R_FOOT: usize = 20;
    pub const R_TOE: usize = 21;
    pub const L_INDEX: usize = 22;
    pub const L_MIDDLE: usize = 23;
    pub const R_INDEX: usize = 24;
    pub const R_MIDDLE: usize = 25;
}
use bone::*;

/// Bones the pose is written to. Toes and the knuckle landmarks keep what
/// Skyrim gives them (the fingers keep their grip on the weapon).
pub fn driven(b: usize) -> bool {
    b <= R_FOOT && b != L_TOE && b != R_TOE
}

/// Elden Ring's baked space to root-local axes (lengths unchanged).
pub fn er_to_skyrim(v: Vec3) -> Vec3 {
    Vec3::new(-v.x, v.z, v.y)
}

/// Joint positions of either skeleton in root-local axes, plus which way the
/// face looks.
pub struct Landmarks {
    pub at: [Vec3; BONE_COUNT],
    pub face: Vec3,
}

impl Landmarks {
    /// From a baked pose.
    pub fn from_er(clips: &Clips, pose: &[f32]) -> Self {
        let mut at = [Vec3::ZERO; BONE_COUNT];
        for (i, &(_, er)) in BONES.iter().enumerate() {
            at[i] = er_to_skyrim(vec3(pose, clips.joint(er)));
        }
        let face = clips.axes_index("Head").map_or(Vec3::Y, |a| er_to_skyrim(vec3(pose, a + 6)));
        Self { at, face }
    }
}

fn norm(v: Vec3) -> Vec3 {
    v.normalize_or_zero()
}

/// The frame with `primary` as its X axis and `secondary`, squared off
/// against it, as its Y axis. None if either is degenerate.
fn frame(primary: Vec3, secondary: Vec3) -> Option<Mat3> {
    let x = primary.try_normalize()?;
    let z = x.cross(secondary).try_normalize()?;
    if x.cross(secondary).length() < 1e-3 * secondary.length().max(1e-6) {
        return None;
    }
    Some(Mat3::from_cols(x, z.cross(x), z))
}

/// Every driven bone's (primary, secondary) pair for one set of landmarks;
/// zero for the rest.
pub fn axes(l: &Landmarks) -> [(Vec3, Vec3); BONE_COUNT] {
    let p = &l.at;
    let hips_left = norm(p[L_THIGH] - p[R_THIGH]);
    // The shoulder joints, not the clavicles' roots: those sit a few
    // centimetres apart at the breastbone and say little.
    let shoulders_left = norm(p[L_UPPERARM] - p[R_UPPERARM]);
    let chest_up = norm(p[NECK] - p[SPINE2]);
    let chest_forward = shoulders_left.cross(chest_up);
    let mut out = [(Vec3::ZERO, Vec3::ZERO); BONE_COUNT];
    out[PELVIS] = (p[SPINE1] - p[PELVIS], hips_left);
    out[SPINE] = (p[SPINE1] - p[SPINE], hips_left + shoulders_left);
    out[SPINE1] = (p[SPINE2] - p[SPINE1], shoulders_left);
    out[SPINE2] = (p[NECK] - p[SPINE2], shoulders_left);
    out[NECK] = (p[HEAD] - p[NECK], shoulders_left);
    out[HEAD] = (p[HEAD] - p[NECK], l.face);
    for (clavicle, upper, fore, hand, index, middle) in
        [(L_CLAVICLE, L_UPPERARM, L_FOREARM, L_HAND, L_INDEX, L_MIDDLE), (R_CLAVICLE, R_UPPERARM, R_FOREARM, R_HAND, R_INDEX, R_MIDDLE)]
    {
        let across = norm(p[index] - p[middle]);
        out[clavicle] = (p[upper] - p[clavicle], chest_forward);
        // Bent, the forearm says how the upper arm is turned; straight, the
        // hand does.
        out[upper] = (p[fore] - p[upper], norm(p[hand] - p[fore]) + across * 0.3);
        out[fore] = (p[hand] - p[fore], across);
        out[hand] = (p[middle] - p[hand], across);
    }
    for (thigh, calf, foot, toe) in [(L_THIGH, L_CALF, L_FOOT, L_TOE), (R_THIGH, R_CALF, R_FOOT, R_TOE)] {
        // A bent knee pushes the knee ahead of the hip-to-ankle line, and the
        // toes point ahead too: both say "forward" for the leg, so the twist
        // never flips as the knee straightens.
        let toes = norm(p[toe] - p[foot]);
        out[thigh] = (p[calf] - p[thigh], norm(p[calf] - p[foot]) + toes * 0.5);
        out[calf] = (p[foot] - p[calf], toes + norm(p[calf] - p[thigh]));
        out[foot] = (p[toe] - p[foot], norm(p[calf] - p[foot]));
    }
    out
}

/// How well each bone's twist is pinned down: the sine of the angle between
/// its primary and secondary axes (0: no twist information at all).
pub fn conditioning(l: &Landmarks) -> [f32; BONE_COUNT] {
    let mut out = [0.0; BONE_COUNT];
    for (o, (p, s)) in out.iter_mut().zip(axes(l)) {
        *o = p.normalize_or_zero().cross(s.normalize_or_zero()).length();
    }
    out
}

/// Every driven bone's frame for one set of landmarks.
pub fn frames(l: &Landmarks) -> [Option<Mat3>; BONE_COUNT] {
    let mut out = [None; BONE_COUNT];
    for (b, (p, s)) in axes(l).into_iter().enumerate() {
        if driven(b) {
            out[b] = frame(p, s);
        }
    }
    out
}

/// The player's skeleton, as read once from Skyrim.
pub struct Binding {
    /// Per bone: rotation from its frame to the bone's own axes, if the bone
    /// is driven and was found.
    offset: [Option<Mat3>; BONE_COUNT],
    /// Skyrim units per baked metre of pelvis height.
    pub scale: f32,
    pub pelvis_ref: Vec3,
    /// What did not work out, by bone, for the log.
    pub missing: Vec<&'static str>,
}

impl Binding {
    /// `reference` holds every bone in root-local space; `found[i]` says
    /// whether the bridge found that node. `er_height` is the baked
    /// standing pelvis height in metres.
    pub fn new(reference: &[ErXform; BONE_COUNT], found: &[bool; BONE_COUNT], er_height: f32) -> Self {
        let mut at = [Vec3::ZERO; BONE_COUNT];
        for i in 0..BONE_COUNT {
            at[i] = reference[i].pos();
        }
        // Skyrim's reference pose looks ahead, along root-local +Y.
        let landmarks = Landmarks { at, face: Vec3::Y };
        let built = frames(&landmarks);
        let mut offset = [None; BONE_COUNT];
        let mut missing = Vec::new();
        for b in 0..BONE_COUNT {
            if !found[b] {
                missing.push(BONES[b].0);
                continue;
            }
            if !driven(b) {
                continue;
            }
            // A frame needs its landmarks found too.
            let needs: &[usize] = match b {
                L_UPPERARM | L_FOREARM | L_HAND => &[L_INDEX, L_MIDDLE, L_FOREARM, L_HAND],
                R_UPPERARM | R_FOREARM | R_HAND => &[R_INDEX, R_MIDDLE, R_FOREARM, R_HAND],
                L_THIGH | L_CALF | L_FOOT => &[L_CALF, L_FOOT, L_TOE],
                R_THIGH | R_CALF | R_FOOT => &[R_CALF, R_FOOT, R_TOE],
                _ => &[PELVIS, SPINE1, SPINE2, NECK, HEAD, L_CLAVICLE, R_CLAVICLE, L_THIGH, R_THIGH],
            };
            if needs.iter().any(|&n| !found[n]) {
                continue;
            }
            if let Some(f) = built[b] {
                offset[b] = Some(f.transpose() * orthonormal(reference[b].mat()));
            }
        }
        let pelvis_ref = reference[PELVIS].pos();
        let scale = if er_height > 0.1 && pelvis_ref.z > 1.0 { pelvis_ref.z / er_height } else { 70.0 };
        Self { offset, scale, pelvis_ref, missing }
    }

    pub fn driven_count(&self) -> usize {
        self.offset.iter().filter(|o| o.is_some()).count()
    }

    pub fn drives(&self, b: usize) -> bool {
        self.offset[b].is_some()
    }

    /// Root-local rotations for the Elden Ring pose; None where a bone is not
    /// driven or its frame is degenerate this frame.
    pub fn solve(&self, clips: &Clips, pose: &[f32]) -> [Option<Mat3>; BONE_COUNT] {
        self.solve_after(clips, pose, &[None; BONE_COUNT])
    }

    /// Like `solve`, carrying on from the last frame's rotations: where a
    /// bone's twist is barely defined this frame (a straight arm, say), it
    /// keeps the twist it had and only swings, so it never spins round.
    pub fn solve_after(&self, clips: &Clips, pose: &[f32], previous: &[Option<Mat3>; BONE_COUNT]) -> [Option<Mat3>; BONE_COUNT] {
        let landmarks = Landmarks::from_er(clips, pose);
        let pairs = axes(&landmarks);
        let built = frames(&landmarks);
        let (weak, firm) = (5f32.to_radians().sin(), 15f32.to_radians().sin());
        let mut out = [None; BONE_COUNT];
        for b in 0..BONE_COUNT {
            let Some(offset) = self.offset[b] else {
                continue;
            };
            let (primary, secondary) = pairs[b];
            let primary = primary.normalize_or_zero();
            let condition = primary.cross(secondary.normalize_or_zero()).length();
            // Swing only, from last frame.
            let swung = previous[b].and_then(|last| {
                let aim = last * (offset.transpose() * Vec3::X);
                (primary != Vec3::ZERO).then(|| Mat3::from_quat(Quat::from_rotation_arc(aim.normalize(), primary)) * last)
            });
            out[b] = match (built[b].map(|f| f * offset), swung) {
                (Some(full), Some(swung)) if condition < firm => {
                    let t = ((condition - weak) / (firm - weak)).clamp(0.0, 1.0);
                    Some(slerp(swung, full, t * t * (3.0 - 2.0 * t)))
                }
                (Some(full), _) => Some(full),
                (None, swung) => swung,
            };
        }
        out
    }

    /// Where the pelvis goes, root-local, for the pose and a lift (Skyrim
    /// units) off the ground.
    pub fn pelvis(&self, clips: &Clips, pose: &[f32], lift: f32) -> Vec3 {
        er_to_skyrim(vec3(pose, clips.joint("Pelvis"))) * self.scale + Vec3::Z * lift
    }
}

/// The nearest rotation to `m` (Skyrim's matrices carry rounding, and a scale
/// on a bone would otherwise leak into the offset).
pub fn orthonormal(m: Mat3) -> Mat3 {
    let x = m.x_axis.normalize_or(Vec3::X);
    let y = (m.y_axis - x * x.dot(m.y_axis)).normalize_or(Vec3::Y);
    let z = x.cross(y);
    Mat3::from_cols(x, y, z)
}

pub fn slerp(a: Mat3, b: Mat3, t: f32) -> Mat3 {
    if t >= 1.0 {
        return b;
    }
    if t <= 0.0 {
        return a;
    }
    let (qa, qb) = (Quat::from_mat3(&orthonormal(a)), Quat::from_mat3(&orthonormal(b)));
    Mat3::from_quat(qa.slerp(qb, t))
}

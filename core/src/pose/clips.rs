//! The player's Elden Ring animations, as baked by upstream's
//! `tools/bake_anims.py` into `player_anims.bin` and installed next to the
//! plugin as `ERCombat.eranims` (the same bytes; `er_anims` checks and copies
//! them). Nothing from the game is compiled in: the file comes from the
//! player's own copy of Elden Ring.
//!
//! ```text
//! "ERAN", u32 joints, u32 oriented, u32 clips
//! joints x name, oriented x name          name: u8 length, UTF-8
//! clips x (name, f32 blend frames, u32 frames,
//!          frames x (joints x f32[3] position, oriented x f32[9] X, Y, Z axes))
//! ```
//!
//! Poses are model space at 30 fps, metres, right-handed with +X on the
//! character's left, +Y up and +Z forward, the origin on the floor under the
//! character. Root motion is not in them: the simulation moves the character.

use std::collections::HashMap;
use std::fmt;

use glam::Vec3;

pub const MAGIC: [u8; 4] = *b"ERAN";

/// Joints the retarget needs. A file without any of them is refused.
pub const REQUIRED: &[&str] = &[
    "Pelvis", "Spine", "Spine1", "Spine2", "Neck", "Head", "L_Clavicle", "L_UpperArm", "L_Forearm", "L_Hand", "R_Clavicle",
    "R_UpperArm", "R_Forearm", "R_Hand", "L_Thigh", "L_Calf", "L_Foot", "L_Toe0", "R_Thigh", "R_Calf", "R_Foot", "R_Toe0",
    "L_Finger1", "L_Finger2", "R_Finger1", "R_Finger2",
];
/// Joints whose axes the retarget needs.
pub const REQUIRED_AXES: &[&str] = &["Head"];

#[derive(Debug, PartialEq, Eq)]
pub enum AnimError {
    BadMagic,
    Truncated,
    /// A joint the retarget needs is not in the file.
    MissingJoint(String),
    /// A clip with no frames, or sizes that make no sense.
    Invalid(&'static str),
    TrailingBytes,
}

impl fmt::Display for AnimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnimError::BadMagic => write!(f, "not a baked animation file (no ERAN header)"),
            AnimError::Truncated => write!(f, "cut short"),
            AnimError::MissingJoint(name) => write!(f, "has no joint {name}"),
            AnimError::Invalid(what) => write!(f, "invalid: {what}"),
            AnimError::TrailingBytes => write!(f, "has bytes after the last clip"),
        }
    }
}

pub struct Clip {
    pub name: String,
    /// Frames to cross-fade over when this clip starts, as authored.
    pub blend: f32,
    pub frames: usize,
    data: Vec<f32>,
}

pub struct Clips {
    pub joints: Vec<String>,
    pub oriented: Vec<String>,
    /// Floats per frame.
    pub stride: usize,
    clips: Vec<Clip>,
    by_name: HashMap<String, usize>,
}

struct Cursor<'a> {
    data: &'a [u8],
    at: usize,
}

impl Cursor<'_> {
    fn bytes(&mut self, n: usize) -> Result<&[u8], AnimError> {
        let end = self.at.checked_add(n).ok_or(AnimError::Truncated)?;
        let slice = self.data.get(self.at..end).ok_or(AnimError::Truncated)?;
        self.at = end;
        Ok(slice)
    }

    fn u32(&mut self) -> Result<u32, AnimError> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    fn f32(&mut self) -> Result<f32, AnimError> {
        Ok(f32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    fn name(&mut self) -> Result<String, AnimError> {
        let len = self.bytes(1)?[0] as usize;
        Ok(String::from_utf8_lossy(self.bytes(len)?).into_owned())
    }
}

impl Clips {
    pub fn read(data: &[u8]) -> Result<Self, AnimError> {
        let mut c = Cursor { data, at: 0 };
        if c.bytes(4).map_err(|_| AnimError::BadMagic)? != MAGIC {
            return Err(AnimError::BadMagic);
        }
        let (joint_count, oriented_count, clip_count) = (c.u32()? as usize, c.u32()? as usize, c.u32()? as usize);
        if joint_count == 0 || joint_count > 256 || oriented_count > 256 {
            return Err(AnimError::Invalid("joint counts"));
        }
        let joints = (0..joint_count).map(|_| c.name()).collect::<Result<Vec<_>, _>>()?;
        let oriented = (0..oriented_count).map(|_| c.name()).collect::<Result<Vec<_>, _>>()?;
        let stride = joints.len() * 3 + oriented.len() * 9;
        let mut clips = Vec::with_capacity(clip_count.min(1 << 16));
        let mut by_name = HashMap::new();
        for _ in 0..clip_count {
            let name = c.name()?;
            let blend = c.f32()?;
            let frames = c.u32()? as usize;
            if frames == 0 || !blend.is_finite() {
                return Err(AnimError::Invalid("a clip without frames"));
            }
            let raw = c.bytes(frames.checked_mul(stride * 4).ok_or(AnimError::Truncated)?)?;
            let data: Vec<f32> = raw.chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect();
            by_name.insert(name.clone(), clips.len());
            clips.push(Clip { name, blend: blend.max(0.0), frames, data });
        }
        if c.at != data.len() {
            return Err(AnimError::TrailingBytes);
        }
        let clips = Self { joints, oriented, stride, clips, by_name };
        for &joint in REQUIRED {
            if clips.joint_index(joint).is_none() {
                return Err(AnimError::MissingJoint(joint.to_string()));
            }
        }
        for &joint in REQUIRED_AXES {
            if clips.axes_index(joint).is_none() {
                return Err(AnimError::MissingJoint(format!("{joint} (axes)")));
            }
        }
        Ok(clips)
    }

    pub fn len(&self) -> usize {
        self.clips.len()
    }

    pub fn is_empty(&self) -> bool {
        self.clips.is_empty()
    }

    pub fn get(&self, name: &str) -> Option<&Clip> {
        self.by_name.get(name).map(|&i| &self.clips[i])
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.clips.iter().map(|c| c.name.as_str())
    }

    /// Index of a joint's position within a pose.
    pub fn joint_index(&self, name: &str) -> Option<usize> {
        self.joints.iter().position(|j| j == name).map(|i| i * 3)
    }

    /// Like `joint_index`, for joints `read` made sure are there.
    pub fn joint(&self, name: &str) -> usize {
        self.joint_index(name).unwrap_or_else(|| panic!("no joint {name}"))
    }

    /// Index of an oriented joint's X axis within a pose; Y and Z follow.
    pub fn axes_index(&self, name: &str) -> Option<usize> {
        self.oriented.iter().position(|j| j == name).map(|i| self.joints.len() * 3 + i * 9)
    }

    /// Writes the pose at `frame` into `out`, interpolating between samples.
    pub fn sample(&self, clip: &Clip, frame: f32, looped: bool, out: &mut Vec<f32>) {
        let last = (clip.frames - 1) as f32;
        let frame = if !frame.is_finite() {
            0.0
        } else if looped && last > 0.0 {
            frame.rem_euclid(last)
        } else {
            frame.clamp(0.0, last)
        };
        let i = (frame.floor() as usize).min(clip.frames - 1);
        let j = (i + 1).min(clip.frames - 1);
        let t = frame - i as f32;
        let (a, b) = (&clip.data[i * self.stride..][..self.stride], &clip.data[j * self.stride..][..self.stride]);
        out.clear();
        out.extend(a.iter().zip(b).map(|(a, b)| a + (b - a) * t));
    }

    /// Builds a file in this format, for tests.
    #[cfg(test)]
    pub fn write(joints: &[&str], oriented: &[&str], clips: &[(&str, f32, Vec<Vec<f32>>)]) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        for n in [joints.len(), oriented.len(), clips.len()] {
            out.extend_from_slice(&(n as u32).to_le_bytes());
        }
        let name = |out: &mut Vec<u8>, s: &str| {
            out.push(s.len() as u8);
            out.extend_from_slice(s.as_bytes());
        };
        for j in joints.iter().chain(oriented) {
            name(&mut out, j);
        }
        for (n, blend, frames) in clips {
            name(&mut out, n);
            out.extend_from_slice(&blend.to_le_bytes());
            out.extend_from_slice(&(frames.len() as u32).to_le_bytes());
            for row in frames {
                for v in row {
                    out.extend_from_slice(&v.to_le_bytes());
                }
            }
        }
        out
    }
}

pub fn vec3(pose: &[f32], at: usize) -> Vec3 {
    Vec3::new(pose[at], pose[at + 1], pose[at + 2])
}

pub fn set_vec3(pose: &mut [f32], at: usize, v: Vec3) {
    pose[at..at + 3].copy_from_slice(&v.to_array());
}

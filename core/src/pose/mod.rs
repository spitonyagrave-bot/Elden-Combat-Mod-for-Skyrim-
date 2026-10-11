//! The pose driver: the Elden Ring animation the simulation calls for,
//! sampled every frame and put onto Skyrim's skeleton.
//!
//! `clips` reads the baked animations, `rig` picks and blends clips the way
//! upstream's sandbox does, `retarget` maps the result onto the player's
//! own Skyrim bones. `Driver` ties them together for the C interface, and
//! fades between Skyrim's pose and Elden Ring's when the driver starts and
//! stops, so switching never snaps.

pub mod clips;
pub mod retarget;
pub mod rig;

use std::path::Path;
use std::sync::OnceLock;

use glam::{Mat3, Vec3};

use clips::{AnimError, Clips};
use retarget::{driven, slerp, Binding, ErXform, BONE_COUNT};
use rig::Rig;

use crate::sim::player::Player;

/// Seconds to fade the Elden Ring pose in when the driver starts, and back
/// out to Skyrim's when it stops.
pub const FADE_SECONDS: f32 = 0.2;
/// The standing clip whose pelvis height scales the pose to the player.
const STAND: &str = "a000_000000";
/// Its pelvis height if the clip is missing, metres.
const STAND_HEIGHT: f32 = 0.93;

static ANIMS: OnceLock<Clips> = OnceLock::new();

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    Read(AnimError),
    AlreadyLoaded,
}

pub fn loaded() -> Option<&'static Clips> {
    ANIMS.get()
}

pub fn install(clips: Clips) -> Result<(), LoadError> {
    ANIMS.set(clips).map_err(|_| LoadError::AlreadyLoaded)
}

pub fn load_file(path: &Path) -> Result<(), LoadError> {
    if ANIMS.get().is_some() {
        return Err(LoadError::AlreadyLoaded);
    }
    let bytes = std::fs::read(path).map_err(LoadError::Io)?;
    install(Clips::read(&bytes).map_err(LoadError::Read)?)
}

/// Pelvis height of the standing pose, metres.
pub fn stand_height(clips: &Clips) -> f32 {
    let Some(clip) = clips.get(STAND) else {
        return STAND_HEIGHT;
    };
    let mut pose = Vec::new();
    clips.sample(clip, 0.0, false, &mut pose);
    let h = pose[clips.joint("Pelvis") + 1];
    if h > 0.3 {
        h
    } else {
        STAND_HEIGHT
    }
}

/// What the simulation is doing, for one frame of the driver.
pub struct Frame<'a> {
    pub player: &'a Player,
    /// Animation frames the simulation's clock is past its last tick.
    pub ahead: f32,
    /// Height off the ground the simulation has the player at (a jump's
    /// arc), Skyrim units.
    pub lift: f32,
}

/// What the driver did this frame.
#[derive(Default)]
pub struct Info {
    pub clip: String,
    pub frame: f32,
    pub weight: f32,
    /// The clip the state called for is not in the file.
    pub missing_clip: bool,
}

pub struct Driver {
    rig: Rig,
    binding: Option<Binding>,
    weight: f32,
    /// The last pose written, to fade out from.
    last: [Option<Mat3>; BONE_COUNT],
    last_pelvis: Option<Vec3>,
}

impl Default for Driver {
    fn default() -> Self {
        Self::new()
    }
}

impl Driver {
    pub fn new() -> Self {
        Self { rig: Rig::new(), binding: None, weight: 0.0, last: [None; BONE_COUNT], last_pelvis: None }
    }

    /// Reads the player's skeleton. Returns how many bones will be driven
    /// (0 without animations).
    pub fn bind(&mut self, reference: &[ErXform; BONE_COUNT], found: &[bool; BONE_COUNT]) -> usize {
        let Some(clips) = loaded() else {
            self.binding = None;
            return 0;
        };
        let binding = Binding::new(reference, found, stand_height(clips));
        let count = binding.driven_count();
        self.binding = Some(binding);
        count
    }

    pub fn binding(&self) -> Option<&Binding> {
        self.binding.as_ref()
    }

    pub fn weight(&self) -> f32 {
        self.weight
    }

    /// One frame. `sim` is None once the driver should hand back to Skyrim:
    /// the pose then fades out. `current` is what Skyrim's animation put on
    /// the bones this frame, root-local. Writes the bones to set into `out`
    /// with `drive` saying which; returns false when nothing is to be written.
    pub fn update(
        &mut self,
        sim: Option<Frame>,
        dt: f32,
        current: &[ErXform; BONE_COUNT],
        out: &mut [ErXform; BONE_COUNT],
        drive: &mut [bool; BONE_COUNT],
        info: &mut Info,
    ) -> bool {
        *drive = [false; BONE_COUNT];
        let (Some(clips), Some(binding)) = (loaded(), self.binding.as_ref()) else {
            self.weight = 0.0;
            return false;
        };
        let step = dt.max(0.0) / FADE_SECONDS;
        match sim {
            Some(frame) => {
                match self.rig.update(frame.player, clips, dt, frame.ahead) {
                    Some(pose) => {
                        self.last = binding.solve_after(clips, pose, &self.last);
                        self.last_pelvis = Some(binding.pelvis(clips, pose, frame.lift.max(0.0)));
                        self.weight = (self.weight + step).min(1.0);
                        info.missing_clip = false;
                    }
                    // A clip the file lacks: hold the last pose, or fade out if there is none.
                    None => {
                        info.missing_clip = true;
                        if self.last_pelvis.is_none() {
                            self.weight = 0.0;
                        }
                    }
                }
                info.clip.clone_from(&self.rig.clip);
                info.frame = self.rig.frame;
            }
            None => {
                self.weight = (self.weight - step).max(0.0);
                if self.weight <= 0.0 {
                    // Start fresh next time: no blend from a stale pose.
                    self.rig = Rig::new();
                    self.last = [None; BONE_COUNT];
                    self.last_pelvis = None;
                }
            }
        }
        info.weight = self.weight;
        if self.weight <= 0.0 {
            return false;
        }
        let t = self.weight * self.weight * (3.0 - 2.0 * self.weight);
        let mut any = false;
        for b in 0..BONE_COUNT {
            if !driven(b) || !binding.drives(b) {
                continue;
            }
            let Some(target) = self.last[b] else {
                continue;
            };
            let now = current[b];
            let mut pos = now.pos();
            if b == retarget::bone::PELVIS {
                if let Some(p) = self.last_pelvis {
                    pos = pos.lerp(p, t);
                }
            }
            out[b] = ErXform::new(slerp(now.mat(), target, t), pos);
            drive[b] = true;
            any = true;
        }
        any
    }
}

#[cfg(test)]
pub(crate) mod tests;

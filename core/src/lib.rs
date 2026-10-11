//! Elden Ring player movement and combat, as a library for Skyrim.
//!
//! `sim` is Funny-Bones' engine-free state machine from
//! ELDEN-RING-Combat-Rewrite, with Bevy's math swapped for `glam`. `ffi` is
//! the C interface the SKSE bridge calls once per Skyrim frame: it takes
//! input and the player's state in Skyrim's own units and axes, steps the
//! simulation at its fixed 60 Hz, and hands back where the player should be,
//! which action is playing and whether a swing is live.

pub mod ffi;
pub mod pose;
pub mod sim;

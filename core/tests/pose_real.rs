//! The pose driver on the real baked animations and the real simulation,
//! when pointed at them: `ER_ANIMS=path/to/player_anims.bin cargo test`.
//! Its own test binary: the animations are loaded once per process.

use er_combat_core::ffi::*;
use er_combat_core::pose::retarget::{driven, ErXform, Landmarks, BONE_COUNT};
use er_combat_core::pose::{self, Driver, Frame, Info};
use glam::{Mat3, Quat};

#[test]
fn every_clip_the_simulation_plays_is_there_and_poses_cleanly() {
    let Some(path) = std::env::var_os("ER_ANIMS") else {
        eprintln!("ER_ANIMS not set: skipping");
        return;
    };
    pose::load_file(std::path::Path::new(&path)).expect("load the baked animations");
    let clips = pose::loaded().unwrap();

    // A Skyrim reference: the standing clip at 70 units to the metre, each
    // bone turned its own way.
    let mut stand = Vec::new();
    clips.sample(clips.get("a000_000000").unwrap(), 0.0, false, &mut stand);
    let landmarks = Landmarks::from_er(clips, &stand);
    let mut reference = [ErXform::default(); BONE_COUNT];
    for b in 0..BONE_COUNT {
        let rot = Mat3::from_quat(Quat::from_euler(glam::EulerRot::ZXY, b as f32 * 0.9, 0.3 * b as f32, -0.2));
        reference[b] = ErXform::new(rot, landmarks.at[b] * 70.0);
    }
    let mut driver = Driver::new();
    let count = driver.bind(&reference, &[true; BONE_COUNT]);
    assert_eq!(count, (0..BONE_COUNT).filter(|&b| driven(b)).count());

    let mut combat = ErCombat::new(ErVec3::default());
    let mut world = ErWorld::default();
    let mut out = [ErXform::default(); BONE_COUNT];
    let mut drive = [false; BONE_COUNT];
    let mut info = Info::default();
    let mut played: Vec<String> = Vec::new();
    let dt = 1.0 / 60.0;
    let mut step = |combat: &mut ErCombat, input: &ErInput, world: &mut ErWorld, played: &mut Vec<String>| {
        let o = combat.update(dt, input, world);
        world.position = o.position;
        world.heading = o.heading;
        let frame = Frame { player: combat.player(), ahead: combat.ahead(), lift: combat.lift() };
        assert!(driver.update(Some(frame), dt, &reference, &mut out, &mut drive, &mut info));
        assert!(!info.missing_clip, "no clip {} in the file", info.clip);
        for b in 0..BONE_COUNT {
            assert!(out[b].rot.iter().chain(&out[b].pos).all(|v| v.is_finite()));
        }
        if played.last() != Some(&info.clip) {
            played.push(info.clip.clone());
        }
    };
    let tap = |b: &mut ErButton| *b = ErButton { held: 1, pressed: 1, released: 0 };
    let up = |b: &mut ErButton| *b = ErButton { held: 0, pressed: 0, released: 1 };
    let idle = ErInput::default();
    let mut input = idle;
    for _ in 0..30 {
        step(&mut combat, &input, &mut world, &mut played);
    }
    // Run, roll, attack chain, guard, jump attack.
    input.move_y = 1.0;
    for _ in 0..40 {
        step(&mut combat, &input, &mut world, &mut played);
    }
    tap(&mut input.dodge);
    step(&mut combat, &input, &mut world, &mut played);
    up(&mut input.dodge);
    step(&mut combat, &input, &mut world, &mut played);
    input = idle;
    for _ in 0..80 {
        step(&mut combat, &input, &mut world, &mut played);
    }
    for _ in 0..3 {
        tap(&mut input.light);
        step(&mut combat, &input, &mut world, &mut played);
        input = idle;
        for _ in 0..50 {
            step(&mut combat, &input, &mut world, &mut played);
        }
    }
    input.guard.held = 1;
    input.guard.pressed = 1;
    for _ in 0..40 {
        step(&mut combat, &input, &mut world, &mut played);
        input.guard.pressed = 0;
    }
    input = idle;
    tap(&mut input.jump);
    step(&mut combat, &input, &mut world, &mut played);
    input = idle;
    for i in 0..120 {
        if i == 10 {
            tap(&mut input.light);
        }
        step(&mut combat, &input, &mut world, &mut played);
        input = idle;
    }
    eprintln!("clips played: {}", played.join(" "));
    assert!(played.len() >= 8, "{played:?}");
}

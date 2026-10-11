//! The retarget on a made-up humanoid (round numbers, nothing from the game).

use glam::{Mat3, Quat, Vec3};

use super::clips::{set_vec3, vec3, Clips, REQUIRED};
use super::retarget::{bone::*, driven, er_to_skyrim, Binding, ErXform, BONES, BONE_COUNT};

/// A standing figure in the baked space: +X left, +Y up, +Z forward, metres.
pub(crate) fn standing() -> Vec<(&'static str, [f32; 3])> {
    let mut joints = vec![
        ("Pelvis", [0.0, 0.95, 0.0]),
        ("Spine", [0.0, 1.05, 0.0]),
        ("Spine1", [0.0, 1.15, 0.01]),
        ("Spine2", [0.0, 1.25, 0.0]),
        ("Neck", [0.0, 1.45, 0.01]),
        ("Head", [0.0, 1.55, 0.04]),
    ];
    for (side, x) in [("L_", 1.0f32), ("R_", -1.0)] {
        let j = |name: &str| -> &'static str { Box::leak(format!("{side}{name}").into_boxed_str()) };
        joints.extend([
            (j("Clavicle"), [0.03 * x, 1.38, 0.0]),
            (j("UpperArm"), [0.18 * x, 1.38, 0.0]),
            (j("Forearm"), [0.30 * x, 1.12, -0.03]),
            (j("Hand"), [0.33 * x, 0.88, 0.04]),
            // Palms in, thumbs forward: the index knuckle is ahead of the middle one.
            (j("Finger1"), [0.335 * x, 0.80, 0.085]),
            (j("Finger2"), [0.335 * x, 0.795, 0.06]),
            (j("Thigh"), [0.1 * x, 0.93, 0.0]),
            (j("Calf"), [0.11 * x, 0.5, 0.04]),
            (j("Foot"), [0.12 * x, 0.1, 0.0]),
            (j("Toe0"), [0.12 * x, 0.0, 0.14]),
        ]);
    }
    joints
}

pub(crate) const HEAD_AXES: [f32; 9] = [0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

/// One frame row from named positions.
pub(crate) fn row(joints: &[(&str, [f32; 3])]) -> Vec<f32> {
    let mut out = Vec::new();
    for name in REQUIRED {
        let at = joints.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("no {name}")).1;
        out.extend_from_slice(&at);
    }
    out.extend_from_slice(&HEAD_AXES);
    out
}

pub(crate) fn clips_file(clips: &[(&str, f32, Vec<Vec<f32>>)]) -> Vec<u8> {
    Clips::write(REQUIRED, &["Head"], clips)
}

fn clips_with(poses: Vec<Vec<f32>>) -> Clips {
    Clips::read(&clips_file(&[("a000_000000", 4.0, vec![row(&standing())]), ("test", 4.0, poses)])).unwrap()
}

/// Turns the joints in `which` about `pivot` by `q` (in the baked space).
fn turn(pose: &mut [f32], clips: &Clips, which: &[&str], pivot: &str, q: Quat) {
    let p = vec3(pose, clips.joint(pivot));
    for name in which {
        let at = clips.joint(name);
        set_vec3(pose, at, p + q * (vec3(pose, at) - p));
    }
}

/// Skyrim's reference: the standing figure 70 units to the metre, every bone
/// turned some arbitrary way about itself (Skyrim's bone axes are its own).
fn skyrim_reference(_clips: &Clips) -> ([ErXform; BONE_COUNT], [bool; BONE_COUNT]) {
    let stand = row(&standing());
    let clips = &clips_with(vec![stand.clone()]);
    let mut reference = [ErXform::default(); BONE_COUNT];
    for (i, &(_, er)) in BONES.iter().enumerate() {
        let pos = er_to_skyrim(vec3(&stand, clips.joint(er))) * 70.0;
        let rot = Mat3::from_quat(Quat::from_euler(glam::EulerRot::XYZ, i as f32 * 0.7, 1.3 - i as f32 * 0.4, i as f32 * 0.25));
        reference[i] = ErXform::new(rot, pos);
    }
    (reference, [true; BONE_COUNT])
}

fn close(a: Mat3, b: Mat3) -> bool {
    (a - b).to_cols_array().iter().all(|d| d.abs() < 1e-3)
}

/// The landmark a bone points at, for the swing check.
fn aim(b: usize) -> usize {
    match b {
        PELVIS => SPINE1,
        SPINE => SPINE1,
        SPINE1 => SPINE2,
        SPINE2 => NECK,
        NECK | HEAD => HEAD,
        L_CLAVICLE => L_UPPERARM,
        L_UPPERARM => L_FOREARM,
        L_FOREARM => L_HAND,
        L_HAND => L_MIDDLE,
        R_CLAVICLE => R_UPPERARM,
        R_UPPERARM => R_FOREARM,
        R_FOREARM => R_HAND,
        R_HAND => R_MIDDLE,
        L_THIGH => L_CALF,
        L_CALF => L_FOOT,
        L_FOOT => L_TOE,
        R_THIGH => R_CALF,
        R_CALF => R_FOOT,
        R_FOOT => R_TOE,
        _ => b,
    }
}

/// From what aim landmark does the swing of bone b start (the head aims from the neck).
fn base(b: usize) -> usize {
    if b == HEAD {
        NECK
    } else {
        b
    }
}

#[test]
fn the_reference_pose_gives_back_skyrims_own_rotations() {
    let clips = clips_with(vec![row(&standing())]);
    let (reference, found) = skyrim_reference(&clips);
    let binding = Binding::new(&reference, &found, 0.95);
    assert_eq!(binding.driven_count(), (0..BONE_COUNT).filter(|&b| driven(b)).count());
    assert!((binding.scale - 70.0).abs() < 1e-3);
    let solved = binding.solve(&clips, &row(&standing()));
    for b in 0..BONE_COUNT {
        if driven(b) {
            assert!(close(solved[b].unwrap(), reference[b].mat()), "{}", BONES[b].0);
        } else {
            assert!(solved[b].is_none());
        }
    }
}

#[test]
fn every_bone_points_where_the_elden_ring_one_does() {
    let clips = clips_with(vec![row(&standing())]);
    let (reference, found) = skyrim_reference(&clips);
    let binding = Binding::new(&reference, &found, 0.95);

    // An overhead swing: right arm up and forward, left arm out, trunk
    // twisted, a knee up, the head turned.
    let mut pose = row(&standing());
    let right_arm = ["R_UpperArm", "R_Forearm", "R_Hand", "R_Finger1", "R_Finger2"];
    turn(&mut pose, &clips, &right_arm, "R_UpperArm", Quat::from_rotation_x(-2.6));
    turn(&mut pose, &clips, &["R_Hand", "R_Finger1", "R_Finger2"], "R_Forearm", Quat::from_rotation_x(-0.9));
    turn(&mut pose, &clips, &["L_Forearm", "L_Hand", "L_Finger1", "L_Finger2"], "L_UpperArm", Quat::from_rotation_z(1.2));
    let upper: Vec<&str> = REQUIRED.iter().copied().filter(|j| !j.contains("Thigh") && !j.contains("Calf") && !j.contains("Foot") && !j.contains("Toe") && *j != "Pelvis" && *j != "Spine").collect();
    turn(&mut pose, &clips, &upper, "Spine1", Quat::from_rotation_y(0.6));
    turn(&mut pose, &clips, &["L_Calf", "L_Foot", "L_Toe0"], "L_Thigh", Quat::from_rotation_x(-1.4));
    turn(&mut pose, &clips, &["L_Foot", "L_Toe0"], "L_Calf", Quat::from_rotation_x(1.6));

    let solved = binding.solve(&clips, &pose);
    let er = super::retarget::Landmarks::from_er(&clips, &pose);
    for b in (0..BONE_COUNT).filter(|&b| driven(b)) {
        let r = solved[b].unwrap_or_else(|| panic!("{} has no rotation", BONES[b].0));
        let (from, to) = (base(b), aim(b));
        // The bone's aim in its own axes, from Skyrim's reference...
        let local = reference[b].mat().transpose() * (reference[to].pos() - reference[from].pos());
        let skyrim = (r * local).normalize();
        let wanted = (er.at[to] - er.at[from]).normalize();
        let degrees = skyrim.dot(wanted).clamp(-1.0, 1.0).acos().to_degrees();
        assert!(degrees < 0.5, "{} points {degrees:.2} degrees off", BONES[b].0);
    }
}

#[test]
fn turning_the_whole_figure_turns_every_bone_alike() {
    let clips = clips_with(vec![row(&standing())]);
    let (reference, found) = skyrim_reference(&clips);
    let binding = Binding::new(&reference, &found, 0.95);
    let mut pose = row(&standing());
    let all: Vec<&str> = REQUIRED.to_vec();
    let q = Quat::from_rotation_y(1.1);
    turn(&mut pose, &clips, &all, "Pelvis", q);
    let at = clips.axes_index("Head").unwrap();
    for axis in 0..3 {
        let v = q * vec3(&pose, at + axis * 3);
        set_vec3(&mut pose, at + axis * 3, v);
    }
    let solved = binding.solve(&clips, &pose);
    // The same turn in Skyrim's axes: about +Z, the other way round (the baked
    // space's up-turn toward +X, its left, is a turn toward Skyrim's left).
    let yaw = Mat3::from_quat(Quat::from_rotation_z(1.1));
    for b in (0..BONE_COUNT).filter(|&b| driven(b)) {
        assert!(close(solved[b].unwrap(), yaw * reference[b].mat()), "{}", BONES[b].0);
    }
    // The pelvis goes where the figure's does, scaled to Skyrim.
    let p = binding.pelvis(&clips, &pose, 10.0);
    assert!((p - Vec3::new(0.0, 0.0, 0.95 * 70.0 + 10.0)).length() < 1e-2, "{p}");
}

#[test]
fn a_skeleton_missing_landmarks_drives_only_what_it_can() {
    let clips = clips_with(vec![row(&standing())]);
    let (reference, mut found) = skyrim_reference(&clips);
    found[L_INDEX] = false;
    found[R_TOE] = false;
    let binding = Binding::new(&reference, &found, 0.95);
    for b in [L_UPPERARM, L_FOREARM, L_HAND, R_THIGH, R_CALF, R_FOOT] {
        assert!(!binding.drives(b), "{}", BONES[b].0);
    }
    for b in [PELVIS, SPINE2, HEAD, R_HAND, L_FOOT] {
        assert!(binding.drives(b), "{}", BONES[b].0);
    }
    assert!(binding.missing.contains(&"NPC L Finger10 [LF10]"));
}

#[test]
fn files_that_are_not_animations_are_refused() {
    use super::clips::AnimError;
    assert_eq!(Clips::read(b"nope").err(), Some(AnimError::BadMagic));
    let mut good = clips_file(&[("a", 1.0, vec![row(&standing())])]);
    assert!(Clips::read(&good).is_ok());
    good.push(0);
    assert_eq!(Clips::read(&good).err(), Some(AnimError::TrailingBytes));
    good.truncate(good.len() - 9);
    assert_eq!(Clips::read(&good).err(), Some(AnimError::Truncated));
    let missing = Clips::write(&["Pelvis"], &[], &[("a", 1.0, vec![vec![0.0; 3]])]);
    assert!(matches!(Clips::read(&missing).err(), Some(AnimError::MissingJoint(_))));
}

/// Every frame of every clip of the real baked animations, when the test is
/// pointed at them (`ER_ANIMS=path/to/player_anims.bin cargo test`): each
/// driven bone gets a rotation, nothing is NaN, and every bone points where
/// its Elden Ring joint does.
#[test]
fn the_real_animations_retarget_cleanly() {
    let Some(path) = std::env::var_os("ER_ANIMS") else {
        eprintln!("ER_ANIMS not set: skipping the check against the real animations");
        return;
    };
    let clips = Clips::read(&std::fs::read(path).expect("read ER_ANIMS")).expect("a baked animation file");
    let (reference, found) = skyrim_reference(&clips);
    let binding = Binding::new(&reference, &found, super::stand_height(&clips));
    let mut pose = Vec::new();
    let (mut frames, mut degenerate, mut worst) = (0usize, 0usize, (0.0f32, String::new()));
    let mut jump = (0.0f32, String::new());
    let mut per_bone: Vec<(f32, String)> = vec![(0.0, String::new()); BONE_COUNT];
    let mut big = [0usize; BONE_COUNT];
    let mut weak = [0usize; BONE_COUNT];
    let mut weakest = [f32::MAX; BONE_COUNT];
    let names: Vec<String> = clips.names().map(str::to_string).collect();
    for name in &names {
        let clip = clips.get(name).unwrap();
        let mut previous: [Option<Mat3>; BONE_COUNT] = [None; BONE_COUNT];
        for f in (0..clip.frames).step_by(2) {
            clips.sample(clip, f as f32, false, &mut pose);
            let solved = binding.solve_after(&clips, &pose, &previous);
            let er = super::retarget::Landmarks::from_er(&clips, &pose);
            frames += 1;
            for (b, c) in super::retarget::conditioning(&er).into_iter().enumerate() {
                if c < 10f32.to_radians().sin() {
                    weak[b] += 1;
                }
                weakest[b] = weakest[b].min(c.clamp(0.0, 1.0).asin());
            }
            for b in (0..BONE_COUNT).filter(|&b| driven(b)) {
                let Some(r) = solved[b] else {
                    degenerate += 1;
                    continue;
                };
                assert!(r.to_cols_array().iter().all(|v| v.is_finite()), "{name} frame {f}: NaN on {}", BONES[b].0);
                // No flips: a bone never turns further in two frames than an
                // arm can swing.
                if let Some(p) = previous[b] {
                    let turn = Quat::from_mat3(&(p.transpose() * r)).to_axis_angle().1.to_degrees();
                    let turn = turn.min(360.0 - turn);
                    if turn > 60.0 {
                        big[b] += 1;
                    }
                    let slot = &mut per_bone[b];
                    if turn > slot.0 {
                        *slot = (turn, format!("{name} f{f}"));
                    }
                    if turn > jump.0 {
                        jump = (turn, format!("{name} frame {f} {}", BONES[b].0));
                    }
                }
                previous[b] = Some(r);
                let (from, to) = (base(b), aim(b));
                let local = reference[b].mat().transpose() * (reference[to].pos() - reference[from].pos());
                let wanted = (er.at[to] - er.at[from]).normalize_or_zero();
                if wanted == Vec3::ZERO {
                    continue;
                }
                let degrees = (r * local).normalize().dot(wanted).clamp(-1.0, 1.0).acos().to_degrees();
                if degrees > worst.0 {
                    worst = (degrees, format!("{name} frame {f} {}", BONES[b].0));
                }
            }
        }
    }
    eprintln!("{} clips, {frames} frames checked, {degenerate} degenerate bone frames, worst swing error {:.3} deg ({})", names.len(), worst.0, worst.1);
    for b in (0..BONE_COUNT).filter(|&b| driven(b)) {
        eprintln!("  {:24} largest 2-frame turn {:6.1} deg ({}), {} over 60", BONES[b].0, per_bone[b].0, per_bone[b].1, big[b]);
    }
    // Fast spins are in the data itself (ER's attacks turn the torso a
    // long way in two frames); what the retarget must not do is lose the
    // twist: its secondary axis must stay clear of the primary one.
    eprintln!("largest turn of one bone between samples two frames apart: {:.1} deg ({})", jump.0, jump.1);
    for b in (0..BONE_COUNT).filter(|&b| driven(b)) {
        eprintln!("  {:24} twist poorly defined (under 10 deg) in {} frames, worst {:.1} deg", BONES[b].0, weak[b], weakest[b].to_degrees());
        assert!(weak[b] * 200 < frames, "{} loses its twist in {} of {frames} frames", BONES[b].0, weak[b]);
    }
    assert!(worst.0 < 0.5, "worst swing error {:.3} deg at {}", worst.0, worst.1);
    assert!(degenerate * 1000 < frames, "{degenerate} degenerate bone frames in {frames}");
}

/// The synthetic clips, installed once for this test process.
fn installed() -> &'static Clips {
    use crate::sim::data::{default_weapon, weapons};
    let stand = row(&standing());
    let idle = super::rig::clip_name(weapons()[default_weapon()].stance[0], 0);
    let mut names = vec!["a000_000000".to_string(), idle];
    names.dedup();
    let list: Vec<(&str, f32, Vec<Vec<f32>>)> = names.iter().map(|n| (n.as_str(), 4.0, vec![stand.clone(), stand.clone()])).collect();
    let _ = super::install(Clips::read(&clips_file(&list)).unwrap());
    super::loaded().unwrap()
}

#[test]
fn the_driver_fades_in_holds_and_fades_back_out() {
    use super::{Driver, Frame, Info, FADE_SECONDS};
    use crate::sim::player::Player;
    let clips = installed();
    let (reference, found) = skyrim_reference(clips);
    let mut driver = Driver::new();
    assert_eq!(driver.bind(&reference, &found), (0..BONE_COUNT).filter(|&b| driven(b)).count());

    let player = Player::new(Vec3::ZERO);
    // Skyrim's own pose this frame: everything turned a quarter about +Z.
    let quarter = Mat3::from_quat(Quat::from_rotation_z(1.0));
    let mut current = reference;
    for x in current.iter_mut() {
        *x = ErXform::new(quarter * x.mat(), x.pos());
    }
    let mut out = [ErXform::default(); BONE_COUNT];
    let mut drive = [false; BONE_COUNT];
    let mut info = Info::default();
    let dt = 1.0 / 60.0;

    // First frame: barely in.
    assert!(driver.update(Some(Frame { player: &player, ahead: 0.0, lift: 0.0 }), dt, &current, &mut out, &mut drive, &mut info));
    assert!(info.weight > 0.0 && info.weight < 0.2, "weight {}", info.weight);
    assert!(!info.missing_clip, "the idle is in the file");
    assert!(drive[PELVIS] && drive[R_HAND] && !drive[L_TOE] && !drive[L_INDEX]);
    let frames_in = (FADE_SECONDS / dt).ceil() as usize + 1;
    for _ in 0..frames_in {
        driver.update(Some(Frame { player: &player, ahead: 0.0, lift: 0.0 }), dt, &current, &mut out, &mut drive, &mut info);
    }
    assert_eq!(info.weight, 1.0);
    // Fully in: the standing pose, whatever Skyrim was doing.
    for b in (0..BONE_COUNT).filter(|&b| driven(b)) {
        assert!(close(out[b].mat(), reference[b].mat()), "{}", BONES[b].0);
    }
    // A jump's lift raises the pelvis.
    driver.update(Some(Frame { player: &player, ahead: 0.0, lift: 30.0 }), dt, &current, &mut out, &mut drive, &mut info);
    assert!((out[PELVIS].pos[2] - (reference[PELVIS].pos[2] + 30.0)).abs() < 1e-2);

    // Off: back to Skyrim's pose over the fade, then nothing written at all.
    let mut wrote = 0;
    while driver.update(None, dt, &current, &mut out, &mut drive, &mut info) {
        wrote += 1;
        assert!(wrote < 100);
    }
    assert!(wrote >= frames_in - 3, "faded out over {wrote} frames");
    assert_eq!(info.weight, 0.0);
    assert!(drive.iter().all(|d| !d));
}

#[test]
fn a_missing_clip_holds_the_last_pose_and_says_so() {
    use super::{Driver, Frame, Info};
    use crate::sim::player::{Player, State};
    let clips = installed();
    let (reference, found) = skyrim_reference(clips);
    let mut driver = Driver::new();
    driver.bind(&reference, &found);
    let mut out = [ErXform::default(); BONE_COUNT];
    let mut drive = [false; BONE_COUNT];
    let mut info = Info::default();
    let mut player = Player::new(Vec3::ZERO);
    // Nothing yet to hold: no pose.
    player.state = State::Dead { t: 0.0 };
    assert!(!driver.update(Some(Frame { player: &player, ahead: 0.0, lift: 0.0 }), 0.016, &reference, &mut out, &mut drive, &mut info));
    assert!(info.missing_clip);
    player.state = State::Ground;
    assert!(driver.update(Some(Frame { player: &player, ahead: 0.0, lift: 0.0 }), 0.016, &reference, &mut out, &mut drive, &mut info));
    player.state = State::Dead { t: 0.0 };
    assert!(driver.update(Some(Frame { player: &player, ahead: 0.0, lift: 0.0 }), 0.016, &reference, &mut out, &mut drive, &mut info));
    assert!(info.missing_clip);
}

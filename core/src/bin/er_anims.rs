//! Checks the baked Elden Ring animations (`player_anims.bin`, from
//! upstream's `tools/bake_anims.py` run on your own copy of the game) and
//! writes them as `ERCombat.eranims` for the plugin to load next to
//! `ERCombat.dll`:
//!
//!     cargo run --release --bin er_anims -- path/to/player_anims.bin ERCombat.eranims [ERCombat.erdata]
//!
//! It reports which clips of the action table are missing from the file (the
//! table compiled in, or the `.erdata` given). The file holds data from your
//! own copy of Elden Ring: keep it out of commits and uploads.

use std::path::Path;
use std::process::ExitCode;

use er_combat_core::pose::{self, clips::Clips, rig::clip_name};
use er_combat_core::sim::{erdata, tables::Tables};

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let (Some(input), Some(output)) = (args.first(), args.get(1)) else {
        eprintln!("usage: er_anims <player_anims.bin> <out.eranims> [table.erdata]");
        return ExitCode::from(2);
    };
    let bytes = match std::fs::read(input) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: can't read {}: {e}", input.to_string_lossy());
            return ExitCode::FAILURE;
        }
    };
    let clips = match Clips::read(&bytes) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {} {e}", input.to_string_lossy());
            return ExitCode::FAILURE;
        }
    };
    let table = match args.get(2) {
        Some(path) => match std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| erdata::read(&b).map_err(|e| e.to_string())) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("error: can't read the table {}: {e}", path.to_string_lossy());
                return ExitCode::FAILURE;
            }
        },
        None => {
            if !cfg!(er_data) {
                eprintln!("warning: checking against the stand-in table (no src/sim/extracted.rs); pass an .erdata to check against the real one");
            }
            Tables::compiled()
        }
    };
    let mut wanted: Vec<String> = table.sources().into_iter().map(str::to_string).collect();
    // Stances and the moves between actions the pose driver plays.
    for w in table.weapons {
        for stance in w.stance {
            wanted.push(clip_name(stance, 0));
            wanted.push(clip_name(stance, 100));
        }
    }
    for id in [20000, 20100, 20200, 22100, 300000, 202040, 4000, 17002] {
        wanted.push(clip_name(0, id));
    }
    wanted.dedup();
    let missing: Vec<&String> = wanted.iter().filter(|n| clips.get(n).is_none() && n.as_str() != "stub").collect();
    if let Err(e) = std::fs::write(output, &bytes) {
        eprintln!("error: can't write {}: {e}", output.to_string_lossy());
        return ExitCode::FAILURE;
    }
    println!(
        "wrote {} ({} bytes, {} clips, {} joints, standing pelvis {:.3} m); {} of {} clips the table plays are present",
        Path::new(output).display(),
        bytes.len(),
        clips.len(),
        clips.joints.len(),
        pose::stand_height(&clips),
        wanted.len() - missing.len(),
        wanted.len()
    );
    if !missing.is_empty() {
        println!("missing (the pose holds its last frame for these): {}", missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
    }
    ExitCode::SUCCESS
}

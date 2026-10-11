//! Writes the action table compiled into this build to an `.erdata` file,
//! for a DLL built without it (as CI's is) to load at run time.
//!
//!     cargo run --release --bin er_dump -- ERCombat.erdata
//!
//! Only worth running with the real `src/sim/extracted.rs` in place: without
//! it this dumps the stand-in. The file holds data from your own copy of
//! Elden Ring, so keep it out of commits and uploads like `extracted.rs`.

use std::process::ExitCode;

use er_combat_core::sim::{erdata, tables::Tables};

fn main() -> ExitCode {
    let Some(path) = std::env::args_os().nth(1) else {
        eprintln!("usage: er_dump <out.erdata>");
        return ExitCode::from(2);
    };
    if !cfg!(er_data) {
        eprintln!("warning: built with the stand-in (no src/sim/extracted.rs, or ER_COMBAT_STUB set); writing the stand-in table");
    }
    let tables = Tables::compiled();
    let bytes = erdata::write(&tables);
    // Never write something this build can't read back identically.
    match erdata::read(&bytes) {
        Ok(back) if back == tables => {}
        Ok(_) => {
            eprintln!("error: the table did not read back identically");
            return ExitCode::FAILURE;
        }
        Err(e) => {
            eprintln!("error: the table did not read back: {e}");
            return ExitCode::FAILURE;
        }
    }
    if let Err(e) = std::fs::write(&path, &bytes) {
        eprintln!("error: can't write {}: {e}", path.to_string_lossy());
        return ExitCode::FAILURE;
    }
    println!(
        "wrote {} ({} bytes, {} weapons, format version {})",
        path.to_string_lossy(),
        bytes.len(),
        tables.weapons.len(),
        erdata::VERSION
    );
    ExitCode::SUCCESS
}

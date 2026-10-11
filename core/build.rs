//! Use the real action table when the player has generated it from their own
//! copy of Elden Ring; otherwise build against the stand-in. Setting
//! `ER_COMBAT_STUB` builds against the stand-in even when the real table is
//! there (to run the stand-in's tests, say).

use std::path::Path;

const GENERATED: &str = "src/sim/extracted.rs";

fn main() {
    println!("cargo:rerun-if-changed={GENERATED}");
    println!("cargo:rerun-if-env-changed=ER_COMBAT_STUB");
    println!("cargo::rustc-check-cfg=cfg(er_data)");
    if std::env::var_os("ER_COMBAT_STUB").is_some() {
        println!("cargo:warning=ER_COMBAT_STUB is set: building with the stand-in action table.");
    } else if Path::new(GENERATED).exists() {
        println!("cargo:rustc-cfg=er_data");
    } else {
        println!("cargo:warning=No {GENERATED}: building with the stand-in action table. See README.md to generate the real one.");
    }
}

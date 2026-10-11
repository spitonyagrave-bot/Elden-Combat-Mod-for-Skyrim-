//! Loading a table through the C interface. Its own test binary, so its own
//! process: a load is global and once only.

use std::ffi::CString;

use er_combat_core::ffi::*;
use er_combat_core::sim::data::{max_stamina, weapons, SwapKind};
use er_combat_core::sim::{erdata, extracted, tables};

#[test]
fn a_dumped_table_loads_once_and_answers_the_same() {
    let compiled = tables::Tables::compiled();
    let path = std::env::temp_dir().join(format!("er_combat_core_test_{}.erdata", std::process::id()));
    std::fs::write(&path, erdata::write(&compiled)).unwrap();
    let c_path = CString::new(path.to_str().unwrap()).unwrap();

    assert!(tables::loaded().is_none());
    assert_eq!(unsafe { er_load_data(c_path.as_ptr()) }, ER_LOAD_OK);
    assert_eq!(tables::loaded(), Some(&compiled));
    assert_eq!(unsafe { er_load_data(c_path.as_ptr()) }, ER_LOAD_ALREADY_LOADED);
    std::fs::remove_file(&path).ok();

    // The accessors now answer from the loaded copy, which says the same.
    assert!(!std::ptr::eq(weapons(), extracted::WEAPONS));
    assert_eq!(weapons(), extracted::WEAPONS);
    assert_eq!(max_stamina(), extracted::MAX_STAMINA);
    assert_eq!(tables::swap(SwapKind::NextLeft), extracted::swap(SwapKind::NextLeft));

    // And the simulation runs on it.
    let combat = er_create(ErVec3::default());
    let input = ErInput { move_y: 1.0, ..Default::default() };
    let world = ErWorld::default();
    let mut out = std::mem::MaybeUninit::<ErOutput>::uninit();
    unsafe {
        er_update(combat, 0.1, &input, &world, out.as_mut_ptr());
        let out = out.assume_init();
        assert!(out.position.y > 0.0);
        assert_eq!(out.stamina, 1.0);
        er_destroy(combat);
    }
}

//! The action table the simulation reads, wherever it came from.
//!
//! What gets compiled in is `extracted.rs` (the real table, generated from
//! your own copy of Elden Ring) or the stand-in `extracted_stub.rs`. The
//! public repository can only ever build the stand-in, so the shipped DLL
//! also takes the real table at run time: `er_dump` writes it to a file (see
//! `erdata.rs` for the format) from a build that has it, and `load_file`
//! reads that file into a global once. Every accessor here prefers what was
//! loaded and falls back to what was compiled in, so with nothing loaded the
//! simulation behaves exactly as before.

use std::path::Path;
use std::sync::OnceLock;

use super::data::{ActionDef, ActionId, AirAttackDef, AttackKind, SwapDef, SwapKind, WeaponInfo};
use super::erdata::{self, ReadError};
use super::extracted;

/// One complete action table, held as flat arrays indexed by the variant
/// lists in `data.rs`.
#[derive(Clone, Debug, PartialEq)]
pub struct Tables {
    pub walk_speed: f32,
    pub run_speed: f32,
    pub run_back_speed: f32,
    pub run_side_speed: f32,
    pub sprint_speed: f32,
    pub crouch_walk_speed: f32,
    pub crouch_run_speed: f32,
    pub max_hp: f32,
    pub max_stamina: f32,
    pub weapons: &'static [WeaponInfo],
    pub default_weapon: usize,
    pub shield: usize,
    pub fist: usize,
    pub torch: usize,
    /// `ActionId::BASE_COUNT` entries, by `ActionId::base_slot`.
    pub(super) base: Vec<Option<ActionDef>>,
    /// By `attack_slot`.
    pub(super) attacks: Vec<Option<ActionDef>>,
    /// By `air_slot`.
    pub(super) air: Vec<Option<AirAttackDef>>,
    /// By weapon.
    pub(super) air_paired: Vec<Option<AirAttackDef>>,
    /// By `SwapKind as usize`.
    pub(super) swaps: Vec<SwapDef>,
}

pub(super) fn attack_slot(weapon: usize, two_hand: bool, kind: AttackKind) -> usize {
    (weapon * 2 + two_hand as usize) * AttackKind::ALL.len() + kind as usize
}

pub(super) fn air_slot(weapon: usize, two_hand: bool, heavy: bool) -> usize {
    (weapon * 2 + two_hand as usize) * 2 + heavy as usize
}

impl Tables {
    /// The table compiled into this build, asked for every input there is.
    pub fn compiled() -> Self {
        let weapons = extracted::WEAPONS;
        let n = weapons.len();
        let mut attacks = vec![None; n * 2 * AttackKind::ALL.len()];
        let mut air = vec![None; n * 2 * 2];
        for weapon in 0..n {
            for two_hand in [false, true] {
                for &kind in AttackKind::ALL {
                    attacks[attack_slot(weapon, two_hand, kind)] = extracted::attack(weapon, two_hand, kind);
                }
                for heavy in [false, true] {
                    air[air_slot(weapon, two_hand, heavy)] = extracted::air_attack(weapon, two_hand, heavy);
                }
            }
        }
        Self {
            walk_speed: extracted::WALK_SPEED,
            run_speed: extracted::RUN_SPEED,
            run_back_speed: extracted::RUN_BACK_SPEED,
            run_side_speed: extracted::RUN_SIDE_SPEED,
            sprint_speed: extracted::SPRINT_SPEED,
            crouch_walk_speed: extracted::CROUCH_WALK_SPEED,
            crouch_run_speed: extracted::CROUCH_RUN_SPEED,
            max_hp: extracted::MAX_HP,
            max_stamina: extracted::MAX_STAMINA,
            weapons,
            default_weapon: extracted::DEFAULT_WEAPON,
            shield: extracted::SHIELD,
            fist: extracted::FIST,
            torch: extracted::TORCH,
            base: ActionId::base_ids().into_iter().map(extracted::base).collect(),
            attacks,
            air,
            air_paired: (0..n).map(extracted::air_paired).collect(),
            swaps: SwapKind::ALL.iter().map(|&kind| extracted::swap(kind)).collect(),
        }
    }

    /// Every animation clip this table plays, each once, in a stable order:
    /// what the pose driver must find in the baked animations.
    pub fn sources(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        let actions = self.base.iter().chain(&self.attacks).flatten().map(|a| a.source);
        let air = self.air.iter().chain(&self.air_paired).flatten().map(|a| a.source);
        let swaps = self.swaps.iter().flat_map(|s| [s.start, s.end]);
        for name in actions.chain(air).chain(swaps) {
            if !out.contains(&name) {
                out.push(name);
            }
        }
        out
    }

    /// Attacks are not in this table (`ActionDef`s for them come from
    /// `attack`), so they get `None`, as from the stand-in.
    pub fn base(&self, id: ActionId) -> Option<ActionDef> {
        self.base[id.base_slot()?]
    }

    pub fn attack(&self, weapon: usize, two_hand: bool, kind: AttackKind) -> Option<ActionDef> {
        if weapon >= self.weapons.len() {
            return None;
        }
        self.attacks[attack_slot(weapon, two_hand, kind)]
    }

    pub fn air_attack(&self, weapon: usize, two_hand: bool, heavy: bool) -> Option<AirAttackDef> {
        if weapon >= self.weapons.len() {
            return None;
        }
        self.air[air_slot(weapon, two_hand, heavy)]
    }

    pub fn air_paired(&self, weapon: usize) -> Option<AirAttackDef> {
        self.air_paired.get(weapon).copied().flatten()
    }

    pub fn swap(&self, kind: SwapKind) -> SwapDef {
        self.swaps[kind as usize]
    }
}

static LOADED: OnceLock<Tables> = OnceLock::new();

/// The table loaded at run time, if any.
pub fn loaded() -> Option<&'static Tables> {
    LOADED.get()
}

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    Read(ReadError),
    /// A table is already loaded; it stays.
    AlreadyLoaded,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Io(e) => write!(f, "can't read the file: {e}"),
            LoadError::Read(e) => write!(f, "{e}"),
            LoadError::AlreadyLoaded => write!(f, "a table is already loaded"),
        }
    }
}

/// Makes `tables` the one every accessor returns from now on. Only the
/// first call takes.
pub fn install(tables: Tables) -> Result<(), LoadError> {
    LOADED.set(tables).map_err(|_| LoadError::AlreadyLoaded)
}

/// Reads a file `er_dump` wrote and installs it. Call it before creating
/// anything that reads the table: a `Player` takes its HP and stamina, for
/// one, when it is made.
pub fn load_file(path: &Path) -> Result<(), LoadError> {
    if LOADED.get().is_some() {
        return Err(LoadError::AlreadyLoaded);
    }
    let bytes = std::fs::read(path).map_err(LoadError::Io)?;
    let tables = erdata::read(&bytes).map_err(LoadError::Read)?;
    install(tables)
}

// --- Accessors: the loaded table, else the compiled one ---------------------

macro_rules! scalar {
    ($($(#[$doc:meta])* $name:ident: $ty:ty = $compiled:ident;)*) => {
        $(
            $(#[$doc])*
            pub fn $name() -> $ty {
                match loaded() {
                    Some(t) => t.$name,
                    None => extracted::$compiled,
                }
            }
        )*
    };
}

scalar! {
    walk_speed: f32 = WALK_SPEED;
    run_speed: f32 = RUN_SPEED;
    run_back_speed: f32 = RUN_BACK_SPEED;
    run_side_speed: f32 = RUN_SIDE_SPEED;
    sprint_speed: f32 = SPRINT_SPEED;
    crouch_walk_speed: f32 = CROUCH_WALK_SPEED;
    crouch_run_speed: f32 = CROUCH_RUN_SPEED;
    max_hp: f32 = MAX_HP;
    max_stamina: f32 = MAX_STAMINA;
    weapons: &'static [WeaponInfo] = WEAPONS;
    default_weapon: usize = DEFAULT_WEAPON;
    /// What the left hand holds. Always the last entry of `weapons`.
    shield: usize = SHIELD;
    /// The bare hand.
    fist: usize = FIST;
    torch: usize = TORCH;
}

pub fn base(id: ActionId) -> Option<ActionDef> {
    match loaded() {
        Some(t) => t.base(id),
        None => extracted::base(id),
    }
}

pub fn attack(weapon: usize, two_hand: bool, kind: AttackKind) -> Option<ActionDef> {
    match loaded() {
        Some(t) => t.attack(weapon, two_hand, kind),
        None => extracted::attack(weapon, two_hand, kind),
    }
}

/// Jump attacks while still in the air.
pub fn air_attack(weapon: usize, two_hand: bool, heavy: bool) -> Option<AirAttackDef> {
    match loaded() {
        Some(t) => t.air_attack(weapon, two_hand, heavy),
        None => extracted::air_attack(weapon, two_hand, heavy),
    }
}

/// The jump attack with a weapon in each hand.
pub fn air_paired(weapon: usize) -> Option<AirAttackDef> {
    match loaded() {
        Some(t) => t.air_paired(weapon),
        None => extracted::air_paired(weapon),
    }
}

/// Grip and weapon changes.
pub fn swap(kind: SwapKind) -> SwapDef {
    match loaded() {
        Some(t) => t.swap(kind),
        None => extracted::swap(kind),
    }
}

//! The `.erdata` file: an action table (`tables::Tables`) on disk, so a DLL
//! built without the real table can load it at run time.
//!
//! Everything is little-endian. `f32`s are stored bit for bit, so a table
//! reads back exactly as it was written.
//!
//! ```text
//! header   magic "ERCDATA\0" (8 bytes), version u32 (= VERSION)
//! layout   u32 ActionId::BASE_COUNT, u32 AttackKind::ALL.len(),
//!          u32 SwapKind::ALL.len()   -- must match the reading build
//! scalars  f32 walk, run, run_back, run_side, sprint, crouch_walk,
//!          crouch_run, max_hp, max_stamina
//! weapons  u32 n, then n x (str name, u8 category, f32 attack, f32 weight,
//!          u8 stance[2])
//!          u32 default_weapon, shield, fist, torch
//! base     BASE_COUNT x opt<action>, in ActionId::base_ids() order
//! attacks  for weapon in 0..n, two_hand in [false, true], kind in
//!          AttackKind::ALL: opt<action>
//! air      for weapon, two_hand, heavy in [false, true]: opt<air>
//! paired   for weapon: opt<air>
//! swaps    for kind in SwapKind::ALL: swap
//! (end of file; trailing bytes are an error)
//!
//! str      u32 byte length, UTF-8 bytes
//! bool     u8, 0 or 1
//! opt<T>   u8 0 (none) or 1 followed by T
//! list<T>  u32 count, then the items
//! action   str name, str source, f32 total, input_from, input_dodge_from,
//!          cancel_light, cancel_heavy, cancel_dodge, cancel_jump,
//!          cancel_guard, cancel_move, cancel_left, iframes.0, iframes.1,
//!          bool jump_frames, f32 stamina, list<hit> hits,
//!          opt<f32 f32> charge, list<f32 f32> no_turn,
//!          list<f32 f32 f32> turn, list<f32 x3> motion
//! hit      f32 from, to, mv, guard_damage, stamina, stop, radius,
//!          list<f32 x6> blade
//! air      f32 from, to, stamina, str source, f32 radius, list<f32 x6> blade
//! swap     str start, str end, f32 start_len, end_len, apply, free_from
//! ```
//!
//! Strings and slices read from a file are leaked to get the `'static`
//! lifetimes the table types carry. A table is loaded once per process.

use super::data::{ActionDef, ActionId, AirAttackDef, AttackKind, Hit, SwapDef, SwapKind, WeaponInfo};
use super::tables::Tables;

pub const MAGIC: [u8; 8] = *b"ERCDATA\0";
pub const VERSION: u32 = 1;

// --- Writing -----------------------------------------------------------------

struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn len(&mut self, n: usize) {
        self.u32(u32::try_from(n).expect("table too large for the file format"));
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32s(&mut self, vs: &[f32]) {
        vs.iter().for_each(|&v| self.f32(v));
    }
    fn bool(&mut self, v: bool) {
        self.u8(v as u8);
    }
    fn str(&mut self, s: &str) {
        self.len(s.len());
        self.0.extend_from_slice(s.as_bytes());
    }
    fn opt<T>(&mut self, v: &Option<T>, mut item: impl FnMut(&mut Self, &T)) {
        self.bool(v.is_some());
        if let Some(v) = v {
            item(self, v);
        }
    }
    fn list<T>(&mut self, vs: &[T], mut item: impl FnMut(&mut Self, &T)) {
        self.len(vs.len());
        vs.iter().for_each(|v| item(self, v));
    }

    fn action(&mut self, a: &ActionDef) {
        self.str(a.name);
        self.str(a.source);
        self.f32s(&[
            a.total,
            a.input_from,
            a.input_dodge_from,
            a.cancel_light,
            a.cancel_heavy,
            a.cancel_dodge,
            a.cancel_jump,
            a.cancel_guard,
            a.cancel_move,
            a.cancel_left,
            a.iframes.0,
            a.iframes.1,
        ]);
        self.bool(a.jump_frames);
        self.f32(a.stamina);
        self.list(a.hits, Self::hit);
        self.opt(&a.charge, |w, c| w.f32s(&[c.0, c.1]));
        self.list(a.no_turn, |w, t| w.f32s(&[t.0, t.1]));
        self.list(a.turn, |w, t| w.f32s(&[t.0, t.1, t.2]));
        self.list(a.motion, |w, m| w.f32s(m));
    }

    fn hit(&mut self, h: &Hit) {
        self.f32s(&[h.from, h.to, h.mv, h.guard_damage, h.stamina, h.stop, h.radius]);
        self.list(h.blade, |w, b| w.f32s(b));
    }

    fn air(&mut self, a: &AirAttackDef) {
        self.f32s(&[a.from, a.to, a.stamina]);
        self.str(a.source);
        self.f32(a.radius);
        self.list(a.blade, |w, b| w.f32s(b));
    }

    fn swap(&mut self, s: &SwapDef) {
        self.str(s.start);
        self.str(s.end);
        self.f32s(&[s.start_len, s.end_len, s.apply, s.free_from]);
    }
}

/// The whole table as the bytes of an `.erdata` file.
pub fn write(t: &Tables) -> Vec<u8> {
    let mut w = Writer(Vec::new());
    w.0.extend_from_slice(&MAGIC);
    w.u32(VERSION);
    w.len(ActionId::BASE_COUNT);
    w.len(AttackKind::ALL.len());
    w.len(SwapKind::ALL.len());
    w.f32s(&[
        t.walk_speed,
        t.run_speed,
        t.run_back_speed,
        t.run_side_speed,
        t.sprint_speed,
        t.crouch_walk_speed,
        t.crouch_run_speed,
        t.max_hp,
        t.max_stamina,
    ]);
    w.list(t.weapons, |w, info| {
        w.str(info.name);
        w.u8(info.category);
        w.f32s(&[info.attack, info.weight]);
        w.u8(info.stance[0]);
        w.u8(info.stance[1]);
    });
    for i in [t.default_weapon, t.shield, t.fist, t.torch] {
        w.len(i);
    }
    // The flat arrays are already in file order.
    for a in t.base.iter().chain(&t.attacks) {
        w.opt(a, Writer::action);
    }
    for a in t.air.iter().chain(&t.air_paired) {
        w.opt(a, Writer::air);
    }
    for s in &t.swaps {
        w.swap(s);
    }
    w.0
}

// --- Reading -----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum ReadError {
    /// Not an `.erdata` file.
    BadMagic,
    /// Written by a different version of the format.
    Version(u32),
    /// Written by a build whose variant lists differ from this one's.
    Layout,
    /// The file ends early.
    Truncated,
    /// Something in the file makes no sense; says what.
    Invalid(&'static str),
    /// Bytes left over after the table.
    TrailingBytes,
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::BadMagic => write!(f, "not an .erdata file"),
            ReadError::Version(v) => write!(f, "format version {v}, this build reads {VERSION}"),
            ReadError::Layout => write!(f, "written by a build with different action lists"),
            ReadError::Truncated => write!(f, "the file ends early"),
            ReadError::Invalid(what) => write!(f, "invalid data: {what}"),
            ReadError::TrailingBytes => write!(f, "unexpected bytes after the table"),
        }
    }
}

impl std::error::Error for ReadError {}

type Result<T> = std::result::Result<T, ReadError>;

struct Reader<'a> {
    rest: &'a [u8],
}

fn leak<T>(v: Vec<T>) -> &'static [T] {
    Box::leak(v.into_boxed_slice())
}

impl<'a> Reader<'a> {
    fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        if n > self.rest.len() {
            return Err(ReadError::Truncated);
        }
        let (head, rest) = self.rest.split_at(n);
        self.rest = rest;
        Ok(head)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.bytes(N)?.try_into().unwrap())
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    /// A count of items at least `min_size` bytes each, checked against what
    /// is left so a corrupt count can't ask for a huge allocation.
    fn len(&mut self, min_size: usize) -> Result<usize> {
        let n = self.u32()? as usize;
        if n.saturating_mul(min_size) > self.rest.len() {
            return Err(ReadError::Truncated);
        }
        Ok(n)
    }
    fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.array()?))
    }
    fn f32s<const N: usize>(&mut self) -> Result<[f32; N]> {
        let mut out = [0.0; N];
        for v in &mut out {
            *v = self.f32()?;
        }
        Ok(out)
    }
    fn bool(&mut self) -> Result<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(ReadError::Invalid("flag is neither 0 nor 1")),
        }
    }
    fn str(&mut self) -> Result<&'static str> {
        let n = self.len(1)?;
        let s = std::str::from_utf8(self.bytes(n)?).map_err(|_| ReadError::Invalid("string is not UTF-8"))?;
        Ok(Box::leak(s.to_owned().into_boxed_str()))
    }
    fn opt<T>(&mut self, item: impl FnOnce(&mut Self) -> Result<T>) -> Result<Option<T>> {
        Ok(if self.bool()? { Some(item(self)?) } else { None })
    }
    fn list<T>(&mut self, min_size: usize, mut item: impl FnMut(&mut Self) -> Result<T>) -> Result<&'static [T]> {
        let n = self.len(min_size)?;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(item(self)?);
        }
        Ok(leak(out))
    }

    fn action(&mut self) -> Result<ActionDef> {
        let name = self.str()?;
        let source = self.str()?;
        let [total, input_from, input_dodge_from, cancel_light, cancel_heavy, cancel_dodge, cancel_jump, cancel_guard, cancel_move, cancel_left, if0, if1] =
            self.f32s()?;
        Ok(ActionDef {
            name,
            source,
            total,
            input_from,
            input_dodge_from,
            cancel_light,
            cancel_heavy,
            cancel_dodge,
            cancel_jump,
            cancel_guard,
            cancel_move,
            cancel_left,
            iframes: (if0, if1),
            jump_frames: self.bool()?,
            stamina: self.f32()?,
            hits: self.list(7 * 4 + 4, Self::hit)?,
            charge: self.opt(|r| r.f32s().map(|[a, b]| (a, b)))?,
            no_turn: self.list(2 * 4, |r| r.f32s().map(|[a, b]| (a, b)))?,
            turn: self.list(3 * 4, |r| r.f32s().map(|[a, b, c]| (a, b, c)))?,
            motion: self.list(3 * 4, Self::f32s)?,
        })
    }

    fn hit(&mut self) -> Result<Hit> {
        let [from, to, mv, guard_damage, stamina, stop, radius] = self.f32s()?;
        Ok(Hit { from, to, mv, guard_damage, stamina, stop, radius, blade: self.list(6 * 4, Self::f32s)? })
    }

    fn air(&mut self) -> Result<AirAttackDef> {
        let [from, to, stamina] = self.f32s()?;
        Ok(AirAttackDef {
            from,
            to,
            stamina,
            source: self.str()?,
            radius: self.f32()?,
            blade: self.list(6 * 4, Self::f32s)?,
        })
    }

    fn swap(&mut self) -> Result<SwapDef> {
        let (start, end) = (self.str()?, self.str()?);
        let [start_len, end_len, apply, free_from] = self.f32s()?;
        Ok(SwapDef { start, end, start_len, end_len, apply, free_from })
    }
}

/// Reads an `.erdata` file's bytes back into a table.
pub fn read(bytes: &[u8]) -> Result<Tables> {
    let mut r = Reader { rest: bytes };
    if r.array::<8>().ok() != Some(MAGIC) {
        return Err(ReadError::BadMagic);
    }
    let version = r.u32()?;
    if version != VERSION {
        return Err(ReadError::Version(version));
    }
    let layout = [r.u32()?, r.u32()?, r.u32()?];
    if layout.map(|n| n as usize) != [ActionId::BASE_COUNT, AttackKind::ALL.len(), SwapKind::ALL.len()] {
        return Err(ReadError::Layout);
    }

    let [walk_speed, run_speed, run_back_speed, run_side_speed, sprint_speed, crouch_walk_speed, crouch_run_speed, max_hp, max_stamina] =
        r.f32s()?;
    let weapons = r.list(4 + 1 + 8 + 2, |r| {
        Ok(WeaponInfo {
            name: r.str()?,
            category: r.u8()?,
            attack: r.f32()?,
            weight: r.f32()?,
            stance: [r.u8()?, r.u8()?],
        })
    })?;
    let n = weapons.len();
    // `Moveset` holds the weapon as a u8.
    if n == 0 || n > 256 {
        return Err(ReadError::Invalid("weapon count"));
    }
    let [default_weapon, shield, fist, torch] = [r.u32()?, r.u32()?, r.u32()?, r.u32()?].map(|i| i as usize);
    if [default_weapon, shield, fist, torch].iter().any(|&i| i >= n) || shield == 0 {
        return Err(ReadError::Invalid("weapon index"));
    }

    let mut actions = |count: usize| (0..count).map(|_| r.opt(Reader::action)).collect::<Result<Vec<_>>>();
    let base = actions(ActionId::BASE_COUNT)?;
    let attacks = actions(n * 2 * AttackKind::ALL.len())?;
    let mut airs = |count: usize| (0..count).map(|_| r.opt(Reader::air)).collect::<Result<Vec<_>>>();
    let air = airs(n * 2 * 2)?;
    let air_paired = airs(n)?;
    let swaps = (0..SwapKind::ALL.len()).map(|_| r.swap()).collect::<Result<Vec<_>>>()?;
    if !r.rest.is_empty() {
        return Err(ReadError::TrailingBytes);
    }

    Ok(Tables {
        walk_speed,
        run_speed,
        run_back_speed,
        run_side_speed,
        sprint_speed,
        crouch_walk_speed,
        crouch_run_speed,
        max_hp,
        max_stamina,
        weapons,
        default_weapon,
        shield,
        fist,
        torch,
        base,
        attacks,
        air,
        air_paired,
        swaps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::data::Moveset;
    use crate::sim::{extracted, tables};

    fn compiled_bytes() -> Vec<u8> {
        write(&Tables::compiled())
    }

    /// Dump what is compiled in, read it back, and ask the result every
    /// question the table answers: it must say exactly what was compiled in.
    #[test]
    fn round_trip_answers_like_the_compiled_table() {
        let t = read(&compiled_bytes()).expect("reads back");
        assert_eq!(t, Tables::compiled());

        assert_eq!(t.walk_speed.to_bits(), extracted::WALK_SPEED.to_bits());
        assert_eq!(t.run_speed.to_bits(), extracted::RUN_SPEED.to_bits());
        assert_eq!(t.run_back_speed.to_bits(), extracted::RUN_BACK_SPEED.to_bits());
        assert_eq!(t.run_side_speed.to_bits(), extracted::RUN_SIDE_SPEED.to_bits());
        assert_eq!(t.sprint_speed.to_bits(), extracted::SPRINT_SPEED.to_bits());
        assert_eq!(t.crouch_walk_speed.to_bits(), extracted::CROUCH_WALK_SPEED.to_bits());
        assert_eq!(t.crouch_run_speed.to_bits(), extracted::CROUCH_RUN_SPEED.to_bits());
        assert_eq!(t.max_hp.to_bits(), extracted::MAX_HP.to_bits());
        assert_eq!(t.max_stamina.to_bits(), extracted::MAX_STAMINA.to_bits());
        assert_eq!(t.weapons, extracted::WEAPONS);
        assert_eq!(
            [t.default_weapon, t.shield, t.fist, t.torch],
            [extracted::DEFAULT_WEAPON, extracted::SHIELD, extracted::FIST, extracted::TORCH]
        );

        for id in ActionId::base_ids() {
            assert_eq!(t.base(id), extracted::base(id), "{id:?}");
        }
        for weapon in 0..extracted::WEAPONS.len() {
            for two_hand in [false, true] {
                for &kind in AttackKind::ALL {
                    assert_eq!(t.attack(weapon, two_hand, kind), extracted::attack(weapon, two_hand, kind), "{weapon} {two_hand} {kind:?}");
                    let id = ActionId::Attack(Moveset { weapon: weapon as u8, two_hand }, kind);
                    assert_eq!(t.base(id), None);
                }
                for heavy in [false, true] {
                    assert_eq!(t.air_attack(weapon, two_hand, heavy), extracted::air_attack(weapon, two_hand, heavy));
                }
            }
            assert_eq!(t.air_paired(weapon), extracted::air_paired(weapon));
        }
        for &kind in SwapKind::ALL {
            assert_eq!(t.swap(kind), extracted::swap(kind));
        }
        // And it writes back to the very same bytes.
        assert_eq!(write(&t), compiled_bytes());
    }

    #[test]
    fn nothing_loaded_means_the_compiled_table() {
        // Tests share a process, and nothing in them loads a table.
        assert!(tables::loaded().is_none());
        assert_eq!(tables::max_stamina(), extracted::MAX_STAMINA);
        assert_eq!(tables::shield(), extracted::SHIELD);
        assert_eq!(tables::swap(SwapKind::NextWeapon), extracted::swap(SwapKind::NextWeapon));
    }

    #[test]
    fn bad_files_are_refused() {
        let good = compiled_bytes();
        assert_eq!(read(b"nope"), Err(ReadError::BadMagic));
        assert_eq!(read(&[]), Err(ReadError::BadMagic));

        let mut version = good.clone();
        version[8] = 99;
        assert_eq!(read(&version), Err(ReadError::Version(99)));

        let mut layout = good.clone();
        layout[12] ^= 1;
        assert_eq!(read(&layout), Err(ReadError::Layout));

        for cut in [16, 40, good.len() / 2, good.len() - 1] {
            assert_eq!(read(&good[..cut]), Err(ReadError::Truncated), "cut at {cut}");
        }

        let mut long = good.clone();
        long.push(0);
        assert_eq!(read(&long), Err(ReadError::TrailingBytes));

        // A weapon count far beyond the file's size.
        let mut huge = good.clone();
        let weapons_at = 8 + 4 + 3 * 4 + 9 * 4;
        huge[weapons_at..weapons_at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(read(&huge), Err(ReadError::Truncated));
    }

    #[test]
    fn missing_file_is_an_io_error() {
        let err = tables::load_file(std::path::Path::new("definitely/not/here.erdata")).unwrap_err();
        assert!(matches!(err, tables::LoadError::Io(_)), "{err:?}");
    }
}

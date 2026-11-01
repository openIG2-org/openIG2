//! Mod rule patches (ADR 0044 §6–§9): a mod's `rules.toml` sets named fields of the tables [`Rules`] reads.

use toml_edit::{DocumentMut, TableLike, Value};

use crate::{layout, Building, Hull, ProblemCurve, Rules, ShipTypeRecord, SpaceWeapon, SpyMission, Tech};

/// A field a patch can set from a TOML value.
trait Field {
    fn set(&mut self, v: &Value) -> Result<(), String>;
}

macro_rules! int_field {
    ($($t:ty),*) => {$(
        impl Field for $t {
            fn set(&mut self, v: &Value) -> Result<(), String> {
                let n = v.as_integer().ok_or_else(|| format!("expected an integer, got {}", v.type_name()))?;
                *self = <$t>::try_from(n).map_err(|_| format!("{n} is out of range ({}..={})", <$t>::MIN, <$t>::MAX))?;
                Ok(())
            }
        }
    )*};
}
int_field!(i32, i16, u8);

impl Field for f32 {
    fn set(&mut self, v: &Value) -> Result<(), String> {
        *self = match v {
            Value::Float(f) => *f.value() as f32,
            Value::Integer(i) => *i.value() as f32,
            _ => return Err(format!("expected a number, got {}", v.type_name())),
        };
        Ok(())
    }
}

impl<T: Field, const N: usize> Field for [T; N] {
    fn set(&mut self, v: &Value) -> Result<(), String> {
        let a = v.as_array().filter(|a| a.len() == N).ok_or_else(|| format!("expected an array of {N}"))?;
        self.iter_mut().zip(a.iter()).enumerate().try_for_each(|(k, (x, v))| x.set(v).map_err(|e| format!("[{k}]: {e}")))
    }
}

/// A table record whose fields a patch names.
trait Record {
    const FIELDS: &'static [&'static str];
    fn field(&mut self, name: &str) -> Option<&mut dyn Field>;
}

macro_rules! record {
    ($t:ty { $($f:ident),* }) => {
        impl Record for $t {
            const FIELDS: &'static [&'static str] = &[$(stringify!($f)),*];
            fn field(&mut self, name: &str) -> Option<&mut dyn Field> {
                match name {
                    $(stringify!($f) => Some(&mut self.$f),)*
                    _ => None,
                }
            }
        }
    };
}
record!(Tech { price, build_time, hp, capacity, prerequisites, lab_levels, research_cost, research_time, firepower });
record!(Building { tech, priority, build_work, max_hp, energy, workers, output, flags });
record!(Hull { class, flags, tech });
record!(SpaceWeapon { kind, reload, damage, accuracy, spread, speed, duration, range, graphic, flags });
record!(ShipTypeRecord { scale, top_speed, accel, turn, size_xy, size_z });
record!(SpyMission { kind, xp, days, cost, min_rank, skill_floor, skill_full, chance });
record!(ProblemCurve { a, b, c, d, rate, weight });

/// `<table>.<id>` → the record, with the table's id range in the error.
fn entry<'a, T>(rows: &'a mut [T], table: &str, id: &str) -> Result<&'a mut T, String> {
    let n = rows.len();
    id.parse::<usize>().ok().and_then(|i| rows.get_mut(i)).ok_or_else(|| format!("{table}.{id}: no such id (0..={})", n.saturating_sub(1)))
}

fn value<'a>(item: &'a toml_edit::Item, at: &str) -> Result<&'a Value, String> {
    item.as_value().ok_or_else(|| format!("{at}: expected a value"))
}

fn fields(body: &dyn TableLike, at: &str) -> Result<Vec<(String, Value)>, String> {
    body.iter().map(|(k, v)| Ok((k.to_owned(), value(v, &format!("{at}.{k}"))?.clone()))).collect()
}

fn entries<'a>(body: &'a dyn TableLike, table: &str) -> Result<Vec<(&'a str, &'a dyn TableLike)>, String> {
    body.iter().map(|(id, e)| Ok((id, e.as_table_like().ok_or_else(|| format!("{table}.{id}: expected a table of fields"))?))).collect()
}

fn records<R: Record>(rows: &mut [R], table: &str, body: &dyn TableLike) -> Result<usize, String> {
    let mut n = 0;
    for (id, e) in entries(body, table)? {
        let at = format!("{table}.{id}");
        let r = entry(rows, table, id)?;
        for (f, v) in fields(e, &at)? {
            let field = r.field(&f).ok_or_else(|| format!("{at}.{f}: unknown field (fields: {})", R::FIELDS.join(", ")))?;
            field.set(&v).map_err(|e| format!("{at}.{f}: {e}"))?;
            n += 1;
        }
    }
    Ok(n)
}

/// A table of `[i32; 4]` rows whose columns `names` name.
fn rows(rows: &mut [[i32; 4]], names: [&str; 4], table: &str, body: &dyn TableLike) -> Result<usize, String> {
    let mut n = 0;
    for (id, e) in entries(body, table)? {
        let at = format!("{table}.{id}");
        let r = entry(rows, table, id)?;
        for (f, v) in fields(e, &at)? {
            let k = names.iter().position(|&c| c == f).ok_or_else(|| format!("{at}.{f}: unknown field (fields: {})", names.join(", ")))?;
            r[k].set(&v).map_err(|e| format!("{at}.{f}: {e}"))?;
            n += 1;
        }
    }
    Ok(n)
}

fn list<T: Field>(list: &mut [T], table: &str, body: &dyn TableLike) -> Result<usize, String> {
    for (i, v) in fields(body, table)? {
        entry(list, table, &i)?.set(&v).map_err(|e| format!("{table}.{i}: {e}"))?;
    }
    Ok(body.len())
}

macro_rules! dispatch {
    ($r:ident, $name:ident, $body:ident;
     records: $($rt:ident),*;
     rows: $($wt:ident => $wn:expr),*;
     lists: $($lt:ident),*) => {
        match $name {
            $(stringify!($rt) => records(&mut $r.$rt, $name, $body),)*
            $(stringify!($wt) => rows(&mut $r.$wt, $wn, $name, $body),)*
            $(stringify!($lt) => list(&mut $r.$lt, $name, $body),)*
            _ => Err(format!(
                "{}: unknown table (tables: {})",
                $name,
                [$(stringify!($rt),)* $(stringify!($wt),)* $(stringify!($lt),)*].join(", ")
            )),
        }
    };
}

impl Tech {
    /// Writes the patchable static fields back into the tech's raw `Talalmanyok0` record (the offsets `from_exe` reads).
    fn write_raw(&self, r: &mut [u8]) {
        for (at, v) in [(20, self.price), (24, self.build_time), (32, self.hp), (52, self.research_cost), (56, self.research_time)] {
            r[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        r[37] = self.capacity;
        r[45..48].copy_from_slice(&self.prerequisites);
        r[49..52].copy_from_slice(&self.lab_levels);
    }
}

impl Rules {
    /// Applies a mod's `rules.toml` (ADR 0044 §6–§8) and returns how many fields it set. On an error (bad TOML, an
    /// unknown table, id or field, a value of the wrong kind or out of range) nothing is changed.
    pub fn apply_patch(&mut self, text: &str) -> Result<usize, String> {
        let doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| e.to_string().trim_end().to_owned())?;
        let mut r = self.clone();
        let mut n = 0;
        for (name, item) in doc.iter() {
            let body = item.as_table_like().ok_or_else(|| format!("{name}: expected a table"))?;
            n += dispatch!(r, name, body;
                records: techs, buildings, hulls, space_weapons, ship_types, spy_missions, problem_curves;
                rows: ground_weapons => ["min", "max", "period", "unknown"],
                      ground_rt_weapons => ["min", "max", "reload", "range"],
                      ground_fortress_weapons => ["min", "max", "reload", "range"];
                lists: race_production_mult, race_research_time_mult, race_research_money_mult, research_size_mult,
                       difficulty_production_mult, difficulty_money_mult, difficulty_development_mult,
                       race_peace_threshold, race_trade_demand, race_fear_threshold, race_ai_production_share,
                       race_ai_military_autobuild, kidnap_morale, radar_ranges, planet_radar_ranges, satellite_prices,
                       tank_speeds, ground_hull_hp, ground_turn_speeds, ground_armour, spy_training_cost,
                       spy_training_days, spy_rank_xp, spy_race_cost_mult, spy_hire_cost_mult)?;
        }
        let stride = layout::TECHS.1;
        for (t, raw) in r.techs.iter().zip(r.tech_defaults.chunks_exact_mut(stride)) {
            t.write_raw(raw);
        }
        *self = r;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Made-up tables: three techs, two buildings, a race list, a ground weapon.
    fn rules() -> Rules {
        let stride = layout::TECHS.1;
        Rules {
            techs: (0..3).map(|index| Tech { index, price: 100 + index as i32, ..Default::default() }).collect(),
            tech_defaults: vec![0xAA; 3 * stride],
            buildings: vec![Building::default(); 2],
            race_production_mult: vec![1.0; 4],
            ground_weapons: vec![[1, 2, 3, 4]],
            ..Default::default()
        }
    }

    #[test]
    fn sets_named_fields_and_the_raw_tech_record() {
        let mut r = rules();
        let n = r
            .apply_patch(
                "[techs.1]\nprice = 900\nlab_levels = [1, 2, 3]\ncapacity = 7\n\
                 [buildings.0]\nenergy = -5\nflags = [1, 0]\n\
                 [race_production_mult]\n2 = 1.5\n3 = 2\n\
                 [ground_weapons.0]\nperiod = 30\n",
            )
            .unwrap();
        assert_eq!(n, 8);
        assert_eq!((r.techs[1].price, r.techs[1].lab_levels, r.techs[1].capacity), (900, [1, 2, 3], 7));
        assert_eq!(r.techs[0].price, 100);
        assert_eq!((r.buildings[0].energy, r.buildings[0].flags), (-5, [1, 0]));
        assert_eq!(r.race_production_mult, [1.0, 1.0, 1.5, 2.0]);
        assert_eq!(r.ground_weapons[0], [1, 2, 30, 4]);
        let raw = &r.tech_defaults[layout::TECHS.1..];
        assert_eq!(i32::from_le_bytes(raw[20..24].try_into().unwrap()), 900);
        assert_eq!((raw[37], &raw[49..52]), (7, &[1, 2, 3][..]));
        // Bytes no field covers stay as they were.
        assert_eq!((raw[0], raw[36], raw[48], raw[60]), (0xAA, 0xAA, 0xAA, 0xAA));
    }

    #[test]
    fn later_patches_win() {
        let mut r = rules();
        r.apply_patch("[techs.2]\nprice = 5\nhp = 9\n").unwrap();
        r.apply_patch("techs.2.price = 6\n").unwrap();
        assert_eq!((r.techs[2].price, r.techs[2].hp), (6, 9));
    }

    #[test]
    fn errors_name_the_table_id_and_field_and_change_nothing() {
        let mut r = rules();
        let err = |r: &mut Rules, t: &str| r.apply_patch(t).unwrap_err();
        assert!(err(&mut r, "[tech.1]\nprice = 1\n").starts_with("tech: unknown table (tables: techs, buildings,"));
        assert_eq!(err(&mut r, "[techs.3]\nprice = 1\n"), "techs.3: no such id (0..=2)");
        assert_eq!(err(&mut r, "[techs.x]\nprice = 1\n"), "techs.x: no such id (0..=2)");
        assert!(err(&mut r, "[techs.0]\nprise = 1\n").starts_with("techs.0.prise: unknown field (fields: price, build_time,"));
        assert_eq!(err(&mut r, "[techs.0]\ncapacity = 300\n"), "techs.0.capacity: 300 is out of range (0..=255)");
        assert_eq!(err(&mut r, "[techs.0]\nprice = 1.5\n"), "techs.0.price: expected an integer, got float");
        assert_eq!(err(&mut r, "[techs.0]\nlab_levels = [1, 2]\n"), "techs.0.lab_levels: expected an array of 3");
        assert_eq!(err(&mut r, "[race_production_mult]\n9 = 1.0\n"), "race_production_mult.9: no such id (0..=3)");
        assert!(err(&mut r, "techs = 3\n").starts_with("techs: expected a table"));
        assert!(err(&mut r, "[techs.0\n").contains("TOML parse error"));
        // The table applied before the error is dropped too.
        err(&mut r, "[techs.0]\nprice = 1\n[buildings.0]\nnope = 1\n");
        assert_eq!(r.techs[0].price, 100);
    }

    #[test]
    fn an_empty_patch_keeps_the_raw_records() {
        let mut r = rules();
        // The raw records agree with the parsed fields, as `from_exe` reads them.
        let raw = r.tech_defaults.clone();
        r.techs.iter().zip(r.tech_defaults.chunks_exact_mut(layout::TECHS.1)).for_each(|(t, b)| t.write_raw(b));
        let synced = r.tech_defaults.clone();
        assert_ne!(raw, synced);
        assert_eq!(r.apply_patch("# nothing\n").unwrap(), 0);
        assert_eq!(r.tech_defaults, synced);
    }
}

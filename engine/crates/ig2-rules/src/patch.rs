//! Mod rule patches (ADR 0044 §6–§9): a mod's `rules.toml` sets named fields of the tables [`Rules`] reads.

use toml_edit::{DocumentMut, TableLike, Value};

use crate::{layout, AddedBuilding, AddedTech, Building, Hull, ProblemCurve, Rules, ShipTypeRecord, SpaceWeapon, SpyMission, Tech};

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
    // `[[techs.new]]` / `[[buildings.new]]` are added entries, not a record id ([`add_new`]).
    let added = matches!(table, "techs" | "buildings");
    body.iter().filter(|(id, e)| !(added && *id == "new" && e.is_array_of_tables())).map(|(id, e)| Ok((id, e.as_table_like().ok_or_else(|| format!("{table}.{id}: expected a table of fields"))?))).collect()
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

/// Research-screen categories (`(t − 1) / 16 + 1`) and their fields.
const CATEGORIES: usize = 12;
const FIELDS: usize = 16;
/// Kinds a new building can't be like (ADR 0069 §3): the defences, which the ground war drives by kind.
const DEFENCES: std::ops::RangeInclusive<usize> = 28..=43;
/// The buildings' techs, the colony pick list's (categories 6..=9).
const BUILDING_TECHS: std::ops::RangeInclusive<usize> = 81..=144;

/// Tech slot `t` is free (ADR 0069 §1): the exe's empty `-` record (static fields and every status zero) that no
/// building prices itself with.
fn free_slot(r: &Rules, t: usize) -> bool {
    let s = layout::TECHS.1;
    let raw = &r.tech_defaults[t * s..(t + 1) * s];
    r.techs[t].name == "-" && raw[20..0x45 + 19].iter().all(|&b| b == 0) && !r.buildings.iter().any(|b| b.tech as usize == t)
}

/// A tech by its number or by the key of an added tech.
fn tech_ref(r: &Rules, v: &Value) -> Result<u8, String> {
    match v.as_str() {
        Some(k) => r.added_techs.iter().find(|a| a.key == k).map(|a| a.index as u8).ok_or_else(|| format!("no added tech \"{k}\"")),
        None => v
            .as_integer()
            .filter(|&t| (0..r.techs.len() as i64).contains(&t))
            .map(|t| t as u8)
            .ok_or_else(|| format!("expected a tech number 0..={} or an added tech's key", r.techs.len() - 1)),
    }
}

/// A building kind by its number or by the key of an added kind.
fn building_ref(r: &Rules, v: &Value) -> Result<u8, String> {
    match v.as_str() {
        Some(k) => r.added_buildings.iter().find(|a| a.key == k).map(|a| a.kind as u8).ok_or_else(|| format!("no added building \"{k}\"")),
        None => v
            .as_integer()
            .filter(|&k| (0..r.buildings.len() as i64).contains(&k))
            .map(|k| k as u8)
            .ok_or_else(|| format!("expected a building kind 0..={} or an added building's key", r.buildings.len() - 1)),
    }
}

fn set_at<T: Clone + Default>(v: &mut Vec<T>, i: usize, x: T) {
    if v.len() <= i {
        v.resize(i + 1, T::default());
    }
    v[i] = x;
}

fn copy_at<T: Clone + Default>(v: &mut Vec<T>, from: usize, to: usize) {
    let x = v[from].clone();
    set_at(v, to, x);
}

fn int_in(v: &Value, range: std::ops::RangeInclusive<i64>) -> Result<i64, String> {
    v.as_integer().filter(|n| range.contains(n)).ok_or_else(|| format!("expected an integer {}..={}", range.start(), range.end()))
}

fn text(v: &Value) -> Result<String, String> {
    v.as_str().map(str::to_owned).ok_or_else(|| format!("expected a string, got {}", v.type_name()))
}

type NewEntry<'a> = (String, String, &'a toml_edit::Table);

/// The `[[<table>.new]]` entries of a patch, each with its key and where it is for errors.
fn new_list<'a>(doc: &'a DocumentMut, table: &str) -> Result<Vec<NewEntry<'a>>, String> {
    let Some(item) = doc.get(table).and_then(|i| i.as_table_like()).and_then(|b| b.get("new")) else { return Ok(Vec::new()) };
    let list = item.as_array_of_tables().ok_or_else(|| format!("{table}.new: expected [[{table}.new]] entries"))?;
    list.iter()
        .enumerate()
        .map(|(i, e)| {
            let key = e.get("key").and_then(|k| k.as_str()).filter(|k| !k.is_empty()).ok_or_else(|| format!("{table}.new[{i}]: needs a key = \"…\""))?;
            Ok((key.to_owned(), format!("{table}.new \"{key}\""), e))
        })
        .collect()
}

/// `[[techs.new]]` and `[[buildings.new]]` (ADR 0069): a new tech takes a free field of its research category, a
/// new building kind is appended and starts as a copy of the original kind it is `like`. Returns the fields set.
fn add_new(r: &mut Rules, doc: &DocumentMut) -> Result<usize, String> {
    let (techs, buildings) = (new_list(doc, "techs")?, new_list(doc, "buildings")?);
    let stride = layout::TECHS.1;
    // Pass 1: slots and kinds, so the entries can name each other by key.
    let mut slots = Vec::new();
    for (key, at, e) in &techs {
        if r.added_techs.iter().any(|a| &a.key == key) {
            return Err(format!("{at}: the key is used twice"));
        }
        let c = e.get("category").and_then(|v| v.as_value()).ok_or_else(|| format!("{at}: needs a category = 1..={CATEGORIES}"))?;
        let c = int_in(c, 1..=CATEGORIES as i64).map_err(|m| format!("{at}.category: {m}"))? as usize;
        let first = (c - 1) * FIELDS;
        let t = match e.get("field").and_then(|v| v.as_value()) {
            Some(f) => {
                let t = first + int_in(f, 1..=FIELDS as i64).map_err(|m| format!("{at}.field: {m}"))? as usize;
                if !free_slot(r, t) {
                    return Err(format!("{at}.field: category {c} field {} is tech {t} ({}), not a free one", t - first, r.techs[t].name));
                }
                t
            }
            None => (first + 1..=first + FIELDS).find(|&t| free_slot(r, t)).ok_or_else(|| format!("{at}: category {c} has no free field"))?,
        };
        // The category's latest AI priority and its first model size, until the entry says otherwise.
        let peers = first + 1..=first + FIELDS;
        let prio = peers.clone().map(|p| r.tech_defaults[p * stride + 0xb7 + 1]).max().unwrap_or(0);
        let size = peers.map(|p| r.research_size_mult.get(p - 1).copied().unwrap_or(0.0)).find(|&s| s != 0.0).unwrap_or(1.0);
        let raw = &mut r.tech_defaults[t * stride..(t + 1) * stride];
        // Unknown (1) to every race until its prerequisites are met (`Research_TalalmanyUjraSzamol`).
        raw[0x46..0x45 + 19].fill(1);
        raw[0xb8..0xb7 + 9].fill(prio);
        set_at(&mut r.research_size_mult, t - 1, size);
        r.techs[t].name = key.clone();
        r.added_techs.push(AddedTech { key: key.clone(), index: t, name: key.clone(), description: Vec::new() });
        slots.push((t, at, *e));
    }
    let mut kinds = Vec::new();
    for (key, at, e) in &buildings {
        if r.added_buildings.iter().any(|a| &a.key == key) {
            return Err(format!("{at}: the key is used twice"));
        }
        let original = r.buildings.len() - r.added_buildings.len();
        let like = e.get("like").and_then(|v| v.as_value()).ok_or_else(|| format!("{at}: needs like = <an original kind>"))?;
        let like = int_in(like, 1..=original as i64 - 1).map_err(|m| format!("{at}.like: {m}"))? as usize;
        if DEFENCES.contains(&like) || r.techs[r.buildings[like].tech as usize].name == "-" {
            return Err(format!("{at}.like: kind {like} can't be copied (the defences {}..={} and the unused kinds can't)", DEFENCES.start(), DEFENCES.end()));
        }
        let kind = r.buildings.len();
        if kind > u8::MAX as usize {
            return Err(format!("{at}: more than {} building kinds", u8::MAX));
        }
        r.buildings.push(Building { index: kind, like: like as u8, ..r.buildings[like].clone() });
        copy_at(&mut r.building_models, like, kind);
        copy_at(&mut r.building_footprints, like, kind);
        copy_at(&mut r.scaffold_heights, like - 1, kind - 1);
        copy_at(&mut r.scaffold_masks, like - 1, kind - 1);
        r.added_buildings.push(AddedBuilding { key: key.clone(), kind });
        kinds.push((kind, like, at, *e));
    }

    // Pass 2: the fields.
    let mut n = 0;
    for (t, at, e) in slots {
        let a = r.added_techs.iter().position(|a| a.index == t).expect("added in pass 1");
        for (f, item) in e.iter() {
            let v = value(item, &format!("{at}.{f}"))?;
            let err = |m: String| format!("{at}.{f}: {m}");
            match f {
                "key" | "category" | "field" => continue,
                "name" => r.added_techs[a].name = text(v).map_err(err)?,
                "description" => {
                    r.added_techs[a].description = match v.as_array() {
                        Some(lines) => lines.iter().map(text).collect::<Result<_, _>>().map_err(err)?,
                        None => vec![text(v).map_err(err)?],
                    };
                    if r.added_techs[a].description.len() > 3 {
                        return Err(err("the research screen shows 3 lines".into()));
                    }
                }
                "ai_priority" => {
                    let p = int_in(v, 0..=255).map_err(err)? as u8;
                    r.tech_defaults[t * stride + 0xb8..t * stride + 0xb7 + 9].fill(p);
                }
                "building" => r.techs[t].building = building_ref(r, v).map_err(err)?,
                "prerequisites" => {
                    let list = v.as_array().filter(|l| l.len() <= 3).ok_or_else(|| err("expected an array of up to 3 techs".into()))?;
                    let mut pre = [0; 3];
                    for (k, p) in list.iter().enumerate() {
                        pre[k] = tech_ref(r, p).map_err(|m| format!("{at}.{f}[{k}]: {m}"))?;
                    }
                    r.techs[t].prerequisites = pre;
                }
                "model" => set_at(&mut r.research_screen.models, t - 1, text(v).map_err(err)?),
                "size" => r.research_size_mult[t - 1].set(v).map_err(err)?,
                _ => {
                    let field = r.techs[t].field(f).ok_or_else(|| {
                        let more = Tech::FIELDS.join(", ");
                        format!("{at}.{f}: unknown field (fields: category, field, name, description, ai_priority, building, model, size, {more})")
                    })?;
                    field.set(v).map_err(err)?;
                }
            }
            n += 1;
        }
    }
    for (kind, like, at, e) in kinds {
        for (f, item) in e.iter() {
            let v = value(item, &format!("{at}.{f}"))?;
            let err = |m: String| format!("{at}.{f}: {m}");
            let m = &mut r.building_models[kind];
            match f {
                "key" | "like" => continue,
                "model" => m.model = text(v).map_err(err)?,
                "preview" => m.preview = text(v).map_err(err)?,
                "scale" => m.scale.set(v).map_err(err)?,
                "preview_scale" => m.preview_scale.set(v).map_err(err)?,
                "tech" => r.buildings[kind].tech = tech_ref(r, v).map_err(err)?,
                _ => {
                    let field = r.buildings[kind].field(f).ok_or_else(|| {
                        format!("{at}.{f}: unknown field (fields: like, model, preview, scale, preview_scale, {})", Building::FIELDS.join(", "))
                    })?;
                    field.set(v).map_err(err)?;
                }
            }
            n += 1;
        }
        // A new tech that gives this kind prices it unless the entry chose a tech; the kind's new tech gives it.
        if r.buildings[kind].tech == r.buildings[like].tech {
            if let Some(a) = r.added_techs.iter().find(|a| r.techs[a.index].building as usize == kind) {
                r.buildings[kind].tech = a.index as u8;
            }
        }
        let t = r.buildings[kind].tech as usize;
        if r.added_techs.iter().any(|a| a.index == t) && r.techs[t].building == 0 {
            r.techs[t].building = kind as u8;
        }
        let b = &r.buildings[kind];
        if !BUILDING_TECHS.contains(&t) {
            let (lo, hi) = (BUILDING_TECHS.start(), BUILDING_TECHS.end());
            return Err(format!("{at}.tech: tech {t} is not a building tech ({lo}..={hi}, research categories 6..=9)"));
        }
        if b.priority > 6 || b.max_hp <= 0 {
            return Err(format!("{at}: needs priority 0..=6 and max_hp above 0"));
        }
    }
    Ok(n)
}

impl Tech {
    /// Writes the patchable static fields back into the tech's raw `Talalmanyok0` record (the offsets `from_exe` reads).
    fn write_raw(&self, r: &mut [u8]) {
        for (at, v) in [(20, self.price), (24, self.build_time), (32, self.hp), (52, self.research_cost), (56, self.research_time)] {
            r[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        r[37] = self.capacity;
        r[39] = self.building;
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
        let mut n = add_new(&mut r, &doc)?;
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

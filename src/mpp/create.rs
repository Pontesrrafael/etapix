//! New `.mpp` files without Project, from a template Project made (`assets/modelo.mpp`, built by
//! `tools/mpp_template.ps1`). Besides the project summary the template holds one record of each
//! kind: a task with a resource, a task without one (and Project's placeholder assignment), a
//! link, a work resource with its calendar, and an assignment.
//!
//! Creating takes two steps. [`create_mpp`] first lays out the structure by cloning those
//! prototypes: one task, link, resource (with its calendar) and assignment per item of the
//! project, with its ids and the fields that define the structure. Then [`update_mpp`] writes
//! every other field and the scheduled dates, the same way it saves edits into any `.mpp`.

use std::collections::{BTreeMap, HashMap, HashSet};

use super::update::{ITEM_VAR_COUNT, set_prop, working_time, put_date, put_f64, put_u16, put_u32, set_flag, F_SUMMARY, PLACEHOLDER_RESOURCE};
use super::*;
use crate::calendar::Calendar;
use crate::model::{Project, ResourceKind};
use crate::mspdi::read_project;

const TEMPLATE: &[u8] = include_bytes!("../../assets/modelo.mpp");
// Template record UIDs.
const PROTO_TASK: u32 = 2;
const PROTO_RESOURCE: u32 = 1;
const PROTO_ASSIGNMENT: u32 = 3;
const PROTO_PLACEHOLDER: u32 = 2;
/// The template's resource calendar (of R1); the base calendar is UID 1.
const PROTO_RESOURCE_CALENDAR: u32 = 3;
const BASE_CALENDAR: u32 = 1;
// Calendar variable data: name and working time, with the entry flags Project gives them.
const CV_NAME: u16 = 1;
const CV_WORKING_TIME: u16 = 8;
const CALENDAR_VAR_FLAGS: u16 = 0x0D40;
const DAY_LEN: usize = 60;
const EXCEPTION_LEN: usize = 92;
/// An exception for a plain range of dates, and the closing value Project writes with it.
const EXCEPTION_DATED: u32 = 1;
const EXCEPTION_RANGE_END: u32 = 2;
// Calendar record (12 bytes): base calendar, resource, calendar UID.
const CAL_BASE: usize = 0;
const CAL_RESOURCE: usize = 4;
const CAL_UID: usize = 8;
// Second blocks hold the GUIDs that tie the tables together: a task's own (at 0), its display
// order (f64 at 16) and its parent's (at 24: the project summary for top-level tasks); a link's
// own, its predecessor's and its successor's; an assignment's own, its task's and its resource's;
// a resource's own with its order and its own again; a resource calendar's resource (twice) and
// its base calendar.
const G_SELF: usize = 0;
const G_ORDER: usize = 16;
const G_PARENT: usize = 24;
const G_SECOND: usize = 16;
const G_THIRD: usize = 32;
const G_RESOURCE_AGAIN: usize = 24;
// Task fields of Project's link cache.
const TV_DRIVER: u16 = 0x4E3;
const TV_ROOTS: u16 = 0x4E4;
const TV_LINKED: u16 = 0x4E5;
/// The value Project gives `TV_LINKED` on every task with predecessors.
const LINKED: [u8; 8] = [1, 0, 0, 0, 1, 0, 0xFF, 0xFF];
/// Entry flags of a task's variable data.
const TASK_VAR_FLAGS: u16 = 0x0B40;
const L_UID: usize = 0;
const L_PREDECESSOR: usize = 4;
const L_SUCCESSOR: usize = 8;
/// Work resources carry this bit in byte 12 of their item.
const RESOURCE_WORK_FLAG: u8 = 0x10;

/// A new `.mpp` holding the project of `xml` (Project XML, as Etapix writes it).
pub fn create_mpp(xml: &str) -> Result<Vec<u8>> {
    let project = read_project(xml).map_err(|e| MppError(e.0))?;
    check_supported(&project)?;
    let mut file = template()?;
    let guids = Guids::of(&file, &project, false);
    let calendars = build_calendars(&mut file, &project, &guids)?;
    lay_out(&mut file, &project, &guids, &calendars, false)?;
    write_options(&mut file, &project)?;
    update_mpp(&file.to_bytes(), xml)
}

/// `original` with the project of `xml` saved into it when tasks, links, resources or
/// assignments were added or removed: the file's own records stay as they are (with their views,
/// formats and fields Etapix does not show), new ones come from the template, and removed ones go.
pub fn restructure_mpp(original: &[u8], xml: &str) -> Result<Vec<u8>> {
    let project = read_project(xml).map_err(|e| MppError(e.0))?;
    let before = read_project(&mpp_to_mspdi(original)?).map_err(|e| MppError(e.0))?;
    check_new_resources(&before, &project)?;
    let mut file = cfb::Compound::parse(original).map_err(|e| MppError(e.0))?;
    let guids = Guids::of(&file, &project, true);
    let calendars = extend_calendars(&mut file, &project, &guids)?;
    lay_out(&mut file, &project, &guids, &calendars, true)?;
    update_mpp(&file.to_bytes(), xml)
}

/// `original` with the project of `xml` saved into it, keeping everything Etapix does not show:
/// edits go into the file's own records ([`update_mpp`]); when the structure changed, records are
/// added and removed ([`restructure_mpp`]).
pub fn save_mpp(original: &[u8], xml: &str) -> Result<Vec<u8>> {
    update_mpp(original, xml).or_else(|edit| restructure_mpp(original, xml).map_err(|structure| MppError(format!("{}; {}", edit.0, structure.0))))
}

/// The records of the project: the file's own (with `keep`) or new ones from the template.
fn lay_out(file: &mut cfb::Compound, p: &Project, guids: &Guids, calendars: &CalendarUids, keep: bool) -> Result<()> {
    build_tasks(file, p, guids, calendars, keep)?;
    build_links(file, p, guids, keep)?;
    build_resources(file, p, guids, calendars, keep)?;
    build_assignments(file, p, guids, keep)
}

/// What a new file cannot hold yet.
fn check_supported(p: &Project) -> Result<()> {
    let unsupported = |what: &str| err(format!("{what}: um .mpp novo com isso ainda precisa do Microsoft Project"));
    if p.resources.iter().any(|r| r.rates.len() > 1) {
        return unsupported("recurso com taxas que mudam com o tempo");
    }
    if p.resources.iter().any(|r| r.kind != ResourceKind::Work) {
        return unsupported("recurso material ou de custo");
    }
    if p.assignments.iter().any(|a| a.overtime_work > 0) {
        return unsupported("horas extras");
    }
    if p.tasks.iter().any(|t| t.baseline.is_some()) {
        return unsupported("linha de base");
    }
    Ok(())
}

/// What new resources in an existing file cannot be yet (its own records keep whatever they have).
fn check_new_resources(before: &Project, after: &Project) -> Result<()> {
    let known: HashSet<i64> = before.resources.iter().map(|r| r.uid).collect();
    let new = Project { resources: after.resources.iter().filter(|r| !known.contains(&r.uid)).cloned().collect(), tasks: Vec::new(), assignments: Vec::new(), ..after.clone() };
    check_supported(&new)
}

/// One record of a table, with its items and variable values, ready to be laid out again.
#[derive(Clone)]
struct Row {
    item: Vec<u8>,
    record: Vec<u8>,
    item2: Vec<u8>,
    record2: Vec<u8>,
    /// (field type, entry flags, value).
    vars: Vec<(u16, u16, Vec<u8>)>,
}

impl Row {
    /// An index entry: 16 bytes starting with a UID of 0xFFFF0000 or more.
    fn is_index(&self) -> bool {
        self.record.len() == 16 && u32_at(&self.record, 0) >= 0xFFFF_0000
    }

    fn is_live(&self) -> bool {
        !self.is_index() && !is_deleted(&self.item)
    }
}

/// A table taken apart into rows, to be rebuilt with other rows.
struct Rows {
    name: &'static str,
    /// Where each record holds the UID its variable values are filed under.
    uid_at: usize,
    meta_header: Vec<u8>,
    meta2_header: Vec<u8>,
    var_header: Vec<u8>,
    rows: Vec<Row>,
    /// Variable values of no row, kept as they are: (uid, type, flags, value).
    other_vars: Vec<(u32, u16, u16, Vec<u8>)>,
}

impl Rows {
    fn load(file: &cfb::Compound, name: &'static str, uid_at: usize) -> Self {
        let get = |s: &str| file.get(&format!("/   114/{name}/{s}")).map(<[u8]>::to_vec).unwrap_or_default();
        let (meta, data, meta2, data2, var_meta, var_data) = (get("FixedMeta"), get("FixedData"), get("Fixed2Meta"), get("Fixed2Data"), get("VarMeta"), get("Var2Data"));
        let second = spans(&meta2, &data2);
        let mut rows: Vec<Row> = spans(&meta, &data)
            .into_iter()
            .enumerate()
            .map(|(k, (item, rec))| {
                let (item2, rec2) = second.get(k).cloned().unwrap_or((0..0, 0..0));
                Row { item: meta[item].to_vec(), record: data[rec].to_vec(), item2: meta2[item2].to_vec(), record2: data2[rec2].to_vec(), vars: Vec::new() }
            })
            .collect();
        let mut by_uid: HashMap<u32, usize> = HashMap::new();
        for (k, r) in rows.iter().enumerate() {
            if r.is_live() {
                by_uid.entry(u32_at(&r.record, uid_at)).or_insert(k);
            }
        }
        let mut other_vars = Vec::new();
        for k in 0..u32_at(&var_meta, 8) as usize {
            let e = 24 + 12 * k;
            let (uid, at, ty, flags) = (u32_at(&var_meta, e), u32_at(&var_meta, e + 4) as usize, u16_at(&var_meta, e + 8), u16_at(&var_meta, e + 10));
            let value = var_data.get(at + 4..at + 4 + u32_at(&var_data, at) as usize).unwrap_or_default().to_vec();
            match by_uid.get(&uid) {
                Some(&row) => rows[row].vars.push((ty, flags, value)),
                None => other_vars.push((uid, ty, flags, value)),
            }
        }
        Rows { name, uid_at, meta_header: meta[..16.min(meta.len())].to_vec(), meta2_header: meta2[..16.min(meta2.len())].to_vec(), var_header: var_meta[..24.min(var_meta.len())].to_vec(), rows, other_vars }
    }

    /// The live row whose UID is `uid`.
    fn prototype(&self, uid: u32) -> Result<Row> {
        self.rows
            .iter()
            .find(|r| r.is_live() && u32_at(&r.record, self.uid_at) == uid)
            .cloned()
            .ok_or_else(|| MppError(format!("modelo sem o registro {uid} em {}", self.name)))
    }

    /// Keeps only the rows `keep` accepts (index entries always stay).
    fn retain(&mut self, keep: impl Fn(&[u8]) -> bool) {
        self.rows.retain(|r| r.is_index() || (r.is_live() && keep(&r.record)));
    }

    fn save(mut self, file: &mut cfb::Compound) -> Result<()> {
        // Each live record's item holds how many variable values it has.
        for r in self.rows.iter_mut().filter(|r| !r.is_index()) {
            let n = r.vars.len() as u16;
            put_u16(&mut r.item, ITEM_VAR_COUNT, n);
        }
        let (meta, data) = Self::lay_out(&self.meta_header, self.rows.iter().map(|r| (&r.item, &r.record)));
        let (meta2, data2) = Self::lay_out(&self.meta2_header, self.rows.iter().map(|r| (&r.item2, &r.record2)));
        let mut var_meta = self.var_header.clone();
        let mut var_data = Vec::new();
        let row_vars = self.rows.iter().flat_map(|r| {
            let uid = if r.is_index() { u32::MAX } else { u32_at(&r.record, self.uid_at) };
            r.vars.iter().map(move |(ty, flags, v)| (uid, *ty, *flags, v))
        });
        let vars: Vec<_> = row_vars.chain(self.other_vars.iter().map(|(u, t, f, v)| (*u, *t, *f, v))).collect();
        for (uid, ty, flags, value) in &vars {
            var_meta.extend_from_slice(&uid.to_le_bytes());
            var_meta.extend_from_slice(&(var_data.len() as u32).to_le_bytes());
            var_meta.extend_from_slice(&ty.to_le_bytes());
            var_meta.extend_from_slice(&flags.to_le_bytes());
            var_data.extend_from_slice(&(value.len() as u32).to_le_bytes());
            var_data.extend_from_slice(value);
        }
        if var_meta.len() >= 24 {
            put_u32(&mut var_meta, 8, vars.len() as u32);
            put_u32(&mut var_meta, 20, var_data.len() as u32);
        }
        let streams = [("FixedMeta", meta), ("FixedData", data), ("Fixed2Meta", meta2), ("Fixed2Data", data2), ("VarMeta", var_meta), ("Var2Data", var_data)];
        for (s, bytes) in streams {
            let path = format!("/   114/{}/{s}", self.name);
            if file.get(&path).is_some() {
                file.set(&path, bytes).map_err(|e| MppError(e.0))?;
            }
        }
        Ok(())
    }

    /// A meta stream (header, then one item per record with the record's offset at +4) and the
    /// data stream it indexes.
    fn lay_out<'a>(header: &[u8], rows: impl Iterator<Item = (&'a Vec<u8>, &'a Vec<u8>)>) -> (Vec<u8>, Vec<u8>) {
        let (mut meta, mut data) = (header.to_vec(), Vec::new());
        let mut count = 0;
        for (item, record) in rows {
            let mut item = item.clone();
            put_u32(&mut item, 4, data.len() as u32);
            meta.extend_from_slice(&item);
            data.extend_from_slice(record);
            count += 1;
        }
        if meta.len() >= 16 {
            put_u32(&mut meta, 8, count);
            put_u32(&mut meta, 12, data.len() as u32);
        }
        (meta, data)
    }
}

type Guid = [u8; 16];

/// A random (version 4) GUID.
fn new_guid() -> Guid {
    use std::hash::{BuildHasher, Hasher};
    let half = || {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
        h.finish().to_le_bytes()
    };
    let mut g: Guid = [half(), half()].concat().try_into().unwrap();
    g[7] = (g[7] & 0x0F) | 0x40;
    g[8] = (g[8] & 0x3F) | 0x80;
    g
}

fn guid_at(b: &[u8], o: usize) -> Guid {
    b.get(o..o + 16).and_then(|s| s.try_into().ok()).unwrap_or_default()
}

fn put_guid(b: &mut [u8], o: usize, g: &Guid) {
    if let Some(s) = b.get_mut(o..o + 16) {
        s.copy_from_slice(g);
    }
}

/// The GUIDs of the project's tasks and resources, by index: the file's own for records it keeps,
/// new ones for the rest.
struct Guids {
    tasks: Vec<Guid>,
    resources: Vec<Guid>,
}

impl Guids {
    fn of(file: &cfb::Compound, p: &Project, keep: bool) -> Self {
        let own = |name: &'static str, uid_at: usize| -> HashMap<u32, Guid> {
            if !keep {
                return HashMap::new();
            }
            let rows = Rows::load(file, name, uid_at);
            rows.rows.iter().filter(|r| r.is_live()).map(|r| (u32_at(&r.record, uid_at), guid_at(&r.record2, G_SELF))).collect()
        };
        let (tasks, resources) = (own("TBkndTask", T_UID), own("TBkndRsc", R_UID));
        Guids {
            tasks: p.tasks.iter().map(|t| tasks.get(&(t.uid as u32)).copied().unwrap_or_else(new_guid)).collect(),
            resources: p.resources.iter().map(|r| resources.get(&(r.uid as u32)).copied().unwrap_or_else(new_guid)).collect(),
        }
    }
}

/// The live rows of a table that `keep` (the file's own records are reused), by key.
fn kept_rows(rows: &Rows, keep: bool, key: impl Fn(&[u8]) -> u64) -> HashMap<u64, Row> {
    if !keep {
        return HashMap::new();
    }
    rows.rows.iter().filter(|r| r.is_live()).map(|r| (key(&r.record), r.clone())).collect()
}

/// A prototype from the template, checked to fit the file's own records of that table.
fn prototype(template: &Rows, file: &Rows, uid: u32) -> Result<Row> {
    let proto = template.prototype(uid)?;
    match file.rows.iter().find(|r| r.is_live()) {
        Some(own) if own.record.len() != proto.record.len() || own.item.len() != proto.item.len() => {
            err(format!("{}: os registros deste arquivo têm outro formato que o do modelo do Etapix", file.name))
        }
        _ => Ok(proto),
    }
}

fn template() -> Result<cfb::Compound> {
    cfb::Compound::parse(TEMPLATE).map_err(|e| MppError(e.0))
}

fn build_tasks(file: &mut cfb::Compound, p: &Project, guids: &Guids, calendars: &CalendarUids, keep: bool) -> Result<()> {
    let mut tasks = Rows::load(file, "TBkndTask", T_UID);
    let proto = prototype(&Rows::load(&template()?, "TBkndTask", T_UID), &tasks, PROTO_TASK)?;
    let mut own = kept_rows(&tasks, keep, |r| u32_at(r, T_UID) as u64);
    tasks.retain(|r| u32_at(r, T_UID) == 0);
    // Project's cache of the link network: the summary lists the tasks without predecessors, and
    // each task with predecessors names one of them.
    let roots: Vec<u32> = p.tasks.iter().filter(|t| !t.summary && t.preds.is_empty()).map(|t| t.uid as u32).collect();
    let summary = tasks.rows.iter_mut().find(|r| r.is_live() && u32_at(&r.record, T_UID) == 0).ok_or_else(|| MppError("arquivo sem a tarefa de resumo do projeto".into()))?;
    set_or_add_var(&mut summary.vars, TV_ROOTS, TASK_VAR_FLAGS, uid_list(&roots));
    let summary_guid = guid_at(&summary.record2, G_SELF);
    for (i, t) in p.tasks.iter().enumerate() {
        let mut row = own.remove(&(t.uid as u64)).unwrap_or_else(|| {
            let mut row = proto.clone();
            put_u32(&mut row.record, T_UID, t.uid as u32);
            put_u32(&mut row.record, T_CALENDAR, t.calendar.map_or(u32::MAX, |c| calendars.tasks[&c]));
            set_var(&mut row.vars, TV_NAME, utf16z(&t.name));
            row
        });
        put_u32(&mut row.record, T_ID, t.id as u32);
        put_u16(&mut row.record, T_OUTLINE_LEVEL, outline_level(p, t.parent) as u16);
        set_flag(&mut row.item, F_SUMMARY, t.summary);
        put_guid(&mut row.record2, G_SELF, &guids.tasks[i]);
        put_f64(&mut row.record2, G_ORDER, (i + 2) as f64);
        put_guid(&mut row.record2, G_PARENT, t.parent.map_or(&summary_guid, |p| &guids.tasks[p]));
        match t.preds.first() {
            Some(l) => {
                set_or_add_var(&mut row.vars, TV_DRIVER, TASK_VAR_FLAGS, uid_list(&[p.tasks[l.pred].uid as u32]));
                set_or_add_var(&mut row.vars, TV_LINKED, TASK_VAR_FLAGS, LINKED.to_vec());
            }
            None => row.vars.retain(|v| v.0 != TV_DRIVER && v.0 != TV_LINKED),
        }
        tasks.rows.push(row);
    }
    tasks.save(file)
}

fn outline_level(p: &Project, mut parent: Option<usize>) -> usize {
    let mut level = 1;
    while let Some(i) = parent {
        level += 1;
        parent = p.tasks[i].parent;
    }
    level
}

fn build_links(file: &mut cfb::Compound, p: &Project, guids: &Guids, keep: bool) -> Result<()> {
    let mut links = Rows::load(file, "TBkndCons", L_UID);
    let template = Rows::load(&template()?, "TBkndCons", L_UID);
    let proto = template.rows.iter().find(|r| r.is_live()).cloned().ok_or_else(|| MppError("modelo sem vínculo".into()))?;
    let pair = |pred: u32, succ: u32| (pred as u64) << 32 | succ as u64;
    let mut own = kept_rows(&links, keep, |r| pair(u32_at(r, L_PREDECESSOR), u32_at(r, L_SUCCESSOR)));
    let mut next_uid = own.values().map(|r| u32_at(&r.record, L_UID)).max().unwrap_or(0) + 1..;
    links.rows.clear();
    let pairs = p.tasks.iter().enumerate().flat_map(|(i, t)| t.preds.iter().map(move |l| (l.pred, i)));
    for (pred, succ) in pairs {
        let (pu, su) = (p.tasks[pred].uid as u32, p.tasks[succ].uid as u32);
        let row = own.remove(&pair(pu, su)).unwrap_or_else(|| {
            let mut row = proto.clone();
            put_u32(&mut row.record, L_UID, next_uid.next().unwrap());
            put_u32(&mut row.record, L_PREDECESSOR, pu);
            put_u32(&mut row.record, L_SUCCESSOR, su);
            put_guid(&mut row.record2, G_SELF, &new_guid());
            put_guid(&mut row.record2, G_SECOND, &guids.tasks[pred]);
            put_guid(&mut row.record2, G_THIRD, &guids.tasks[succ]);
            row
        });
        links.rows.push(row);
    }
    links.save(file)
}

/// Calendar UIDs given to the resources' calendars (by resource UID) and to the base calendars
/// tasks use (by index in the project's calendars).
struct CalendarUids {
    resources: BTreeMap<i64, u32>,
    tasks: HashMap<usize, u32>,
}

/// A new file's calendars: the project calendar (the template's base calendar, with the
/// project's working time), one more base calendar per calendar a task uses, and one calendar per
/// resource, derived from the project calendar.
fn build_calendars(file: &mut cfb::Compound, p: &Project, guids: &Guids) -> Result<CalendarUids> {
    let mut calendars = Rows::load(file, "TBkndCal", CAL_UID);
    let resource_proto = calendar_row(&calendars, PROTO_RESOURCE_CALENDAR)?;
    calendars.rows.retain(|r| r.record.len() != 12 || u32_at(&r.record, CAL_UID) != PROTO_RESOURCE_CALENDAR);
    let base = calendars.rows.iter_mut().find(|r| r.record.len() == 12 && u32_at(&r.record, CAL_UID) == BASE_CALENDAR).ok_or_else(|| MppError("modelo sem calendário base".into()))?;
    let project_calendar = &p.calendars[p.calendar];
    set_working_time(base, project_calendar);
    let base_proto = base.clone();
    let mut next_uid = PROTO_RESOURCE_CALENDAR..;
    let mut out = CalendarUids { resources: BTreeMap::new(), tasks: HashMap::new() };
    for ((r, guid), uid) in p.resources.iter().zip(&guids.resources).zip(&mut next_uid) {
        let mut row = resource_calendar(&resource_proto, r.uid, uid, guid);
        // A resource's own working time; without it the calendar follows the project's.
        if let Some(own) = r.calendar.map(|c| &p.calendars[c]).filter(|c| working_time(c) != working_time(project_calendar)) {
            set_working_time(&mut row, own);
        }
        calendars.rows.push(row);
        out.resources.insert(r.uid, uid);
    }
    out.tasks.insert(p.calendar, BASE_CALENDAR);
    let mut task_calendars: Vec<usize> = p.tasks.iter().filter_map(|t| t.calendar).filter(|&c| c != p.calendar).collect();
    task_calendars.sort_unstable();
    task_calendars.dedup();
    for (c, uid) in task_calendars.into_iter().zip(&mut next_uid) {
        let mut row = base_proto.clone();
        put_u32(&mut row.record, CAL_UID, uid);
        put_guid(&mut row.record2, G_SELF, &new_guid());
        set_or_add_var(&mut row.vars, CV_NAME, CALENDAR_VAR_FLAGS, utf16z(&p.calendars[c].name));
        set_working_time(&mut row, &p.calendars[c]);
        calendars.rows.push(row);
        out.tasks.insert(c, uid);
    }
    calendars.save(file)?;
    Ok(out)
}

/// An existing file's calendars stay as they are; a new resource gets a calendar derived from the
/// project calendar, and a removed resource's calendar goes with it.
fn extend_calendars(file: &mut cfb::Compound, p: &Project, guids: &Guids) -> Result<CalendarUids> {
    let mut calendars = Rows::load(file, "TBkndCal", CAL_UID);
    let proto = calendar_row(&Rows::load(&template()?, "TBkndCal", CAL_UID), PROTO_RESOURCE_CALENDAR)?;
    let resource_of = |r: &Row| (r.record.len() == 12).then(|| i32_at(&r.record, CAL_RESOURCE)).filter(|&res| res > 0);
    let uids: HashSet<i64> = p.resources.iter().map(|r| r.uid).collect();
    calendars.rows.retain(|r| resource_of(r).is_none_or(|res| uids.contains(&(res as i64))));
    let mut out = CalendarUids { resources: BTreeMap::new(), tasks: HashMap::new() };
    for r in &calendars.rows {
        if let Some(res) = resource_of(r) {
            out.resources.insert(res as i64, u32_at(&r.record, CAL_UID));
        }
    }
    let mut next_uid = calendars.rows.iter().filter(|r| r.record.len() == 12).map(|r| u32_at(&r.record, CAL_UID)).max().unwrap_or(0) + 1..;
    for (r, guid) in p.resources.iter().zip(&guids.resources) {
        if let std::collections::btree_map::Entry::Vacant(slot) = out.resources.entry(r.uid) {
            let uid = next_uid.next().unwrap();
            calendars.rows.push(resource_calendar(&proto, r.uid, uid, guid));
            slot.insert(uid);
        }
    }
    calendars.save(file)?;
    Ok(out)
}

fn calendar_row(calendars: &Rows, uid: u32) -> Result<Row> {
    calendars.rows.iter().find(|r| r.record.len() == 12 && u32_at(&r.record, CAL_UID) == uid).cloned().ok_or_else(|| MppError("modelo sem calendário de recurso".into()))
}

/// A resource's calendar, derived from the project calendar.
fn resource_calendar(proto: &Row, resource: i64, uid: u32, guid: &Guid) -> Row {
    let mut row = proto.clone();
    put_guid(&mut row.record2, G_SELF, guid);
    put_guid(&mut row.record2, G_SECOND, guid);
    put_u32(&mut row.record, CAL_BASE, BASE_CALENDAR);
    put_u32(&mut row.record, CAL_RESOURCE, resource as u32);
    put_u32(&mut row.record, CAL_UID, uid);
    row
}

/// A calendar's working time as Project stores it: seven weekday blocks (Sunday first), the
/// number of exceptions at 420, the exceptions (92 bytes each, from 424), and 4 closing bytes.
fn calendar_blob(c: &Calendar) -> Vec<u8> {
    // A day as Project's default week has it is stored as "default" (type 1), as Project does.
    let standard = Calendar::standard();
    let mut out: Vec<u8> = c.weekdays.iter().zip(&standard.weekdays).flat_map(|(d, s)| if d == s { default_block() } else { shift_block(d) }).collect();
    out.extend_from_slice(&(c.exceptions.len() as u32).to_le_bytes());
    for e in &c.exceptions {
        let mut entry = [0u8; EXCEPTION_LEN];
        put_u16(&mut entry, 0, (e.from - epoch()).num_days() as u16);
        put_u16(&mut entry, 2, (e.to - epoch()).num_days() as u16);
        put_u32(&mut entry, 4, 1); // occurrences
        entry[12..12 + DAY_LEN].copy_from_slice(&shift_block(&e.shifts));
        put_u32(&mut entry, 72, EXCEPTION_DATED);
        put_u32(&mut entry, 80, EXCEPTION_RANGE_END);
        out.extend_from_slice(&entry);
    }
    out.extend_from_slice(&[0; 4]);
    out
}

/// A day that follows the default week.
fn default_block() -> [u8; DAY_LEN] {
    let mut b = [0u8; DAY_LEN];
    put_u16(&mut b, 0, 1);
    b
}

/// One day's working time: type 0 (own times), the number of shifts, the working total, then
/// for each shift its start (u16 at 8), length (u32 at 20) and the work done by its end (u32 at
/// 40), in tenths of a minute. Project ignores a day whose running totals are missing.
fn shift_block(shifts: &[(u32, u32)]) -> [u8; DAY_LEN] {
    let mut b = [0u8; DAY_LEN];
    let shifts = &shifts[..shifts.len().min(5)];
    put_u16(&mut b, 2, shifts.len() as u16);
    put_u32(&mut b, 4, shifts.iter().map(|(a, z)| (z - a) * 10).sum());
    let mut done = 0;
    for (i, (a, z)) in shifts.iter().enumerate() {
        done += (z - a) * 10;
        put_u16(&mut b, 8 + 2 * i, (a * 10) as u16);
        put_u32(&mut b, 20 + 4 * i, (z - a) * 10);
        put_u32(&mut b, 40 + 4 * i, done);
    }
    b
}

fn utf16z(s: &str) -> Vec<u8> {
    s.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect()
}

fn build_resources(file: &mut cfb::Compound, p: &Project, guids: &Guids, calendars: &CalendarUids, keep: bool) -> Result<()> {
    let mut resources = Rows::load(file, "TBkndRsc", R_UID);
    let proto = prototype(&Rows::load(&template()?, "TBkndRsc", R_UID), &resources, PROTO_RESOURCE)?;
    let mut own = kept_rows(&resources, keep, |r| u32_at(r, R_UID) as u64);
    resources.retain(|r| u32_at(r, R_UID) == 0);
    for ((id, r), guid) in (1..).zip(&p.resources).zip(&guids.resources) {
        let mut row = own.remove(&(r.uid as u64)).unwrap_or_else(|| {
            // A new resource: what defines it; update_mpp leaves resources alone.
            let mut row = proto.clone();
            put_guid(&mut row.record2, G_SELF, guid);
            put_guid(&mut row.record2, G_RESOURCE_AGAIN, guid);
            put_u32(&mut row.record, R_UID, r.uid as u32);
            let rate = r.rate_at(p.start);
            put_f64(&mut row.record, R_MAX_UNITS, r.max_units * 10000.0);
            put_f64(&mut row.record, R_STANDARD_RATE, rate.standard);
            put_f64(&mut row.record, R_OVERTIME_RATE, rate.overtime);
            put_f64(&mut row.record, R_COST_PER_USE, rate.per_use * 100.0);
            put_u32(&mut row.record, R_CALENDAR, calendars.resources[&r.uid]);
            if let Some(b) = row.item.get_mut(12) {
                *b = if r.kind == ResourceKind::Work { *b | RESOURCE_WORK_FLAG } else { *b & !RESOURCE_WORK_FLAG };
            }
            set_var(&mut row.vars, RV_NAME, utf16z(&r.name));
            row
        });
        put_u32(&mut row.record, R_ID, id);
        put_f64(&mut row.record2, G_ORDER, (id + 1) as f64);
        resources.rows.push(row);
    }
    resources.save(file)
}

/// The assignments of the project, and Project's placeholder assignment for each task without
/// resources. Work and dates are left to [`update_mpp`].
fn build_assignments(file: &mut cfb::Compound, p: &Project, guids: &Guids, keep: bool) -> Result<()> {
    let mut assignments = Rows::load(file, "TBkndAssn", A_UID);
    let template = Rows::load(&template()?, "TBkndAssn", A_UID);
    let (real, placeholder) = (prototype(&template, &assignments, PROTO_ASSIGNMENT)?, prototype(&template, &assignments, PROTO_PLACEHOLDER)?);
    let pair = |task: u32, resource: u32| (task as u64) << 32 | resource as u64;
    let mut own = kept_rows(&assignments, keep, |r| pair(u32_at(r, A_TASK), u32_at(r, A_RESOURCE)));
    let mut uid = own.values().map(|r| u32_at(&r.record, A_UID)).max().unwrap_or(0) + 1..;
    assignments.rows.clear();
    // A placeholder keeps the template's GUID for "no resource".
    let mut add = |proto: &Row, task: usize, resource: Option<usize>| {
        let (tu, ru) = (p.tasks[task].uid as u32, resource.map_or(PLACEHOLDER_RESOURCE, |r| p.resources[r].uid as u32));
        own.remove(&pair(tu, ru)).unwrap_or_else(|| {
            let mut row = proto.clone();
            put_u32(&mut row.record, A_UID, uid.next().unwrap());
            put_u32(&mut row.record, A_TASK, tu);
            put_u32(&mut row.record, A_RESOURCE, ru);
            put_guid(&mut row.record2, G_SELF, &new_guid());
            put_guid(&mut row.record2, G_SECOND, &guids.tasks[task]);
            if let Some(r) = resource {
                put_guid(&mut row.record2, G_THIRD, &guids.resources[r]);
            }
            row
        })
    };
    for a in &p.assignments {
        let row = add(&real, a.task, Some(a.resource));
        assignments.rows.push(row);
    }
    for (i, t) in p.tasks.iter().enumerate() {
        if !t.summary && !p.assignments.iter().any(|a| a.task == i) {
            let row = add(&placeholder, i, None);
            assignments.rows.push(row);
        }
    }
    assignments.save(file)
}

/// Project options, in the project's `Props`.
fn write_options(file: &mut cfb::Compound, p: &Project) -> Result<()> {
    let path = "/   114/Props";
    let mut props = file.get(path).map(<[u8]>::to_vec).ok_or_else(|| MppError("modelo sem Props".into()))?;
    set_prop(&mut props, PROJECT_START, |v| put_date(v, 0, Some(p.start)));
    set_prop(&mut props, STATUS_DATE, |v| put_date(v, 0, p.status_date));
    set_prop(&mut props, CRITICAL_SLACK_LIMIT, |v| put_u32(v, 0, p.critical_slack_limit as u32));
    // Stored the way Project XML has it: 0 when tasks honor their constraints.
    set_prop(&mut props, HONOR_CONSTRAINTS, |v| v[0] = !p.honor_constraints as u8);
    set_prop(&mut props, MULTIPLE_CRITICAL_PATHS, |v| v[0] = p.multiple_critical_paths as u8);
    file.set(path, props).map_err(|e| MppError(e.0))
}

/// A count, then the first UIDs: Project keeps at most five, and fails to open a file with more.
fn uid_list(uids: &[u32]) -> Vec<u8> {
    const KEPT: usize = 5;
    std::iter::once(uids.len() as u32).chain(uids.iter().copied().take(KEPT)).flat_map(u32::to_le_bytes).collect()
}

fn set_or_add_var(vars: &mut Vec<(u16, u16, Vec<u8>)>, ty: u16, flags: u16, value: Vec<u8>) {
    match vars.iter_mut().find(|v| v.0 == ty) {
        Some(v) => v.2 = value,
        None => vars.push((ty, flags, value)),
    }
}

fn set_var(vars: &mut [(u16, u16, Vec<u8>)], ty: u16, value: Vec<u8>) {
    if let Some(v) = vars.iter_mut().find(|v| v.0 == ty) {
        v.2 = value;
    }
}

/// Gives a calendar record its own working time; its item says so (bit 6 of byte 8), or Project
/// ignores the stored times and uses the defaults.
fn set_working_time(row: &mut Row, c: &Calendar) {
    set_or_add_var(&mut row.vars, CV_WORKING_TIME, CALENDAR_VAR_FLAGS, calendar_blob(c));
    set_flag(&mut row.item, (8, 6), true);
}

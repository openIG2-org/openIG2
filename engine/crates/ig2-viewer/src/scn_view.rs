use eframe::egui;
use ig2_formats::scn::{Entry, Record, RecordKind, Scenario};
use ig2_formats::Install;

#[derive(Default)]
pub struct ScnView {
    current: Option<(String, Result<Scenario, String>)>,
    search: String,
}

const KINDS: [(RecordKind, &str); 8] = [
    (RecordKind::Planet, "PLANET (bolygó)"),
    (RecordKind::Fleet, "FLEET (flotta)"),
    (RecordKind::DummyFleet, "DUMMYFLEET"),
    (RecordKind::Terv, "TERV (design)"),
    (RecordKind::Talalmany, "TALALMANY (technology)"),
    (RecordKind::Spy, "SPY (kém)"),
    (RecordKind::Viszony, "VISZONY (relation)"),
    (RecordKind::Building, "BUILDING"),
];

fn entry_row(ui: &mut egui::Ui, e: &Entry) {
    ui.monospace(&e.key);
    ui.monospace(&e.raw);
    ui.weak(format!("line {}", e.line));
    ui.end_row();
}

fn record_ui(ui: &mut egui::Ui, r: &Record, idx: usize, filter: &str) {
    let title = r
        .get("pNev")
        .or_else(|| r.get("fNev"))
        .or_else(|| r.get("tNev"))
        .or_else(|| r.get("sNev"))
        .map(|e| e.raw.clone())
        .unwrap_or_else(|| r.header.raw.clone());
    if !filter.is_empty() && !title.to_ascii_lowercase().contains(filter) {
        return;
    }
    egui::CollapsingHeader::new(format!("#{idx}  {title}")).id_salt(("rec", idx, r.header.line)).show(ui, |ui| {
        egui::Grid::new(("grid", r.header.line)).striped(true).show(ui, |ui| {
            entry_row(ui, &r.header);
            for f in &r.fields {
                entry_row(ui, f);
            }
        });
        for (ci, c) in r.children.iter().enumerate() {
            record_ui(ui, c, ci, "");
        }
    });
}

impl ScnView {
    pub fn ui(&mut self, ui: &mut egui::Ui, install: &Install, path: &str) {
        if self.current.as_ref().map(|(p, _)| p.as_str()) != Some(path) {
            let res = install.read(path).map(|b| Scenario::parse_bytes(&b, install.lang())).map_err(|e| e.to_string());
            self.current = Some((path.to_owned(), res));
        }
        let Some((_, res)) = &self.current else { return };
        let scn = match res {
            Ok(s) => s,
            Err(e) => {
                ui.colored_label(egui::Color32::LIGHT_RED, e);
                return;
            }
        };
        ui.horizontal(|ui| {
            ui.heading(scn.global("Title").map(|e| e.raw.as_str()).unwrap_or("(untitled)"));
            ui.separator();
            ui.label("Find record:");
            ui.text_edit_singleline(&mut self.search);
        });
        let filter = self.search.to_ascii_lowercase();
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            egui::CollapsingHeader::new(format!("Globals ({})", scn.globals.len())).default_open(true).show(ui, |ui| {
                egui::Grid::new("globals").striped(true).show(ui, |ui| {
                    for e in &scn.globals {
                        entry_row(ui, e);
                    }
                });
            });
            for (kind, label) in KINDS {
                let recs: Vec<&Record> = scn.records_of(kind).collect();
                if recs.is_empty() {
                    continue;
                }
                egui::CollapsingHeader::new(format!("{label} ({})", recs.len())).show(ui, |ui| {
                    for (i, r) in recs.iter().enumerate() {
                        record_ui(ui, r, i, &filter);
                    }
                });
            }
            if !scn.comments.is_empty() {
                egui::CollapsingHeader::new(format!("Comments ({})", scn.comments.len())).show(ui, |ui| {
                    for (line, c) in &scn.comments {
                        ui.monospace(format!("{line}: {c}"));
                    }
                });
            }
        });
    }
}

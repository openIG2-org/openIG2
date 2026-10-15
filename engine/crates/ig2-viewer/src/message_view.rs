use eframe::egui;
use ig2_formats::dialogue::{self, Block, MessageKind, Speaker, TextPart};
use ig2_formats::{text, Install};

struct Loaded {
    kind: Option<MessageKind>,
    parts: Vec<TextPart>,
    blocks: Option<Vec<Block>>,
}

#[derive(Default)]
pub struct MessageView {
    index: Option<(String, Vec<MessageKind>)>,
    current: crate::Cached<Loaded>,
}

fn read(install: &Install, path: &str) -> Result<String, String> {
    install.read(path).map(|b| text::decode(&b, install.lang())).map_err(|e| e.to_string())
}

impl MessageView {
    pub fn ui(&mut self, ui: &mut egui::Ui, install: &Install, path: &str) {
        let root_key = format!("{}|{:?}", install.root().display(), install.lang());
        if self.index.as_ref().map(|(k, _)| k != &root_key).unwrap_or(true) {
            let idx = read(install, "dataraw/uzenetek/uzenetek.txt").map(|t| dialogue::parse_message_index(&t)).unwrap_or_default();
            self.index = Some((root_key, idx));
            self.current = None;
        }
        if self.current.as_ref().map(|(p, _)| p.as_str()) != Some(path) {
            let num = path.rsplit('/').next().unwrap_or("").trim_start_matches("txt").trim_end_matches(".txt").to_owned();
            let id: u32 = num.parse().unwrap_or(0);
            let res = read(install, path).map(|t| Loaded {
                kind: self.index.as_ref().and_then(|(_, i)| i.iter().find(|k| k.id == id).cloned()),
                parts: dialogue::parse_txt_script(&t),
                blocks: read(install, &format!("dataraw/uzenetek/msg{num}.txt")).ok().map(|m| dialogue::parse_msg_script(&m)),
            });
            self.current = Some((path.to_owned(), res));
        }
        let Some((_, res)) = &self.current else { return };
        let l = match res {
            Ok(l) => l,
            Err(e) => {
                ui.colored_label(egui::Color32::LIGHT_RED, e);
                return;
            }
        };
        if let Some(k) = &l.kind {
            ui.heading(format!("{} — {} {}", k.sender, k.subject[0], k.subject[1]));
            ui.label(format!(
                "conversation {}   automatic {}   replaces {}",
                k.conversation, k.automatic, k.replaces
            ));
        } else {
            ui.colored_label(egui::Color32::YELLOW, "not in uzenetek.txt (unreachable in game)");
        }
        ui.separator();
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            if let Some(blocks) = &l.blocks {
                egui::CollapsingHeader::new(format!("Dialogue blocks ({})", blocks.len())).default_open(true).show(ui, |ui| {
                    for b in blocks {
                        ui.monospace(format!("block {:02}", b.id));
                        for c in &b.choices {
                            let part = if c.part == -1 { "auto".to_owned() } else { format!("part {:02}", c.part) };
                            let next = if c.next == -1 { "end".to_owned() } else { format!("block {:02}", c.next) };
                            ui.monospace(format!("    [{part} → {next}] {}", c.label));
                        }
                    }
                });
            }
            for p in &l.parts {
                let who = match p.speaker {
                    Speaker::Player => "player (EN)",
                    Speaker::Other => "other (TE)",
                };
                let map = p.map_anim.map(|m| format!("  story map #{m}")).unwrap_or_default();
                egui::CollapsingHeader::new(format!("part {:02} — {who}  speed {}  anim {}{map}", p.part, p.speed, p.anim_start))
                    .id_salt(("part", p.part, p.anim_start, p.lines.len()))
                    .default_open(true)
                    .show(ui, |ui| {
                        let mut para = String::new();
                        for line in &p.lines {
                            if line.new_line && !para.is_empty() {
                                ui.label(std::mem::take(&mut para));
                            }
                            if !para.is_empty() {
                                para.push(' ');
                            }
                            para.push_str(&line.text);
                        }
                        if !para.is_empty() {
                            ui.label(para);
                        }
                    });
            }
        });
    }
}

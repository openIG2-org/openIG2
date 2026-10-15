use eframe::egui;
use ig2_formats::text::Lang;
use ig2_formats::Install;

use crate::ani_view::AniView;
use crate::colony_view::ColonyView;
use crate::message_view::MessageView;
use crate::model_view::ModelView;
use crate::scn_view::ScnView;
use crate::texture_view::TextureView;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Textures,
    Models,
    Colony,
    Scenarios,
    Animations,
    Messages,
}

impl Tab {
    const ALL: [(Tab, &'static str); 6] = [
        (Tab::Textures, "Textures"),
        (Tab::Models, "Models"),
        (Tab::Colony, "Colony maps"),
        (Tab::Scenarios, "Scenarios"),
        (Tab::Animations, "Animations"),
        (Tab::Messages, "Messages"),
    ];

    fn matches(self, path: &str) -> bool {
        match self {
            Tab::Textures => path.ends_with(".pvr"),
            Tab::Models => path.ends_with(".xv"),
            Tab::Colony => path.starts_with("dataraw/colony/gen_maps/") && path.ends_with(".map"),
            Tab::Scenarios => path.ends_with(".scn"),
            Tab::Animations => path.ends_with(".ani"),
            Tab::Messages => path.starts_with("dataraw/uzenetek/txt") && path.ends_with(".txt"),
        }
    }
}

pub struct App {
    install_input: String,
    install: Option<Install>,
    error: Option<String>,
    lang: Lang,
    tab: Tab,
    filter: String,
    files: Vec<String>,
    selected: Option<String>,
    textures: TextureView,
    models: ModelView,
    colony: ColonyView,
    scn: ScnView,
    anis: AniView,
    messages: MessageView,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, install: Option<String>) -> Self {
        let mut app = Self {
            install_input: install.clone().unwrap_or_default(),
            install: None,
            error: None,
            lang: Lang::En,
            tab: Tab::Textures,
            filter: String::new(),
            files: Vec::new(),
            selected: None,
            textures: TextureView::default(),
            models: ModelView::new(cc),
            colony: ColonyView::default(),
            scn: ScnView::default(),
            anis: AniView::default(),
            messages: MessageView::default(),
        };
        if install.is_some() {
            app.open_install();
        }
        app
    }

    fn open_install(&mut self) {
        match Install::open(self.install_input.trim(), self.lang) {
            Ok(i) => {
                self.install = Some(i);
                self.error = None;
                self.refresh_files();
            }
            Err(e) => {
                self.install = None;
                self.error = Some(e.to_string());
            }
        }
    }

    fn refresh_files(&mut self) {
        self.files = match &self.install {
            Some(i) => i.list("").into_iter().filter(|p| self.tab.matches(p)).map(str::to_owned).collect(),
            None => Vec::new(),
        };
        self.selected = None;
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Install:");
            let edit = ui.add(egui::TextEdit::singleline(&mut self.install_input).desired_width(420.0));
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("Open").clicked() || enter {
                self.open_install();
            }
            ui.separator();
            ui.label("Language:");
            let before = self.lang;
            egui::ComboBox::from_id_salt("lang").selected_text(format!("{:?}", self.lang)).show_ui(ui, |ui| {
                for l in Lang::ALL {
                    ui.selectable_value(&mut self.lang, l, format!("{l:?}"));
                }
            });
            if before != self.lang && self.install.is_some() {
                self.open_install();
            }
            ui.separator();
            for (tab, label) in Tab::ALL {
                if ui.selectable_label(self.tab == tab, label).clicked() && self.tab != tab {
                    self.tab = tab;
                    self.refresh_files();
                }
            }
        });
        if let Some(e) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, e);
        }
    }

    fn file_list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.text_edit_singleline(&mut self.filter);
        });
        let needle = self.filter.to_ascii_lowercase();
        let shown: Vec<&String> = self.files.iter().filter(|f| f.contains(&needle)).collect();
        ui.label(format!("{} / {} files", shown.len(), self.files.len()));
        ui.separator();
        let row_h = ui.text_style_height(&egui::TextStyle::Body);
        let mut clicked = None;
        egui::ScrollArea::vertical().auto_shrink(false).show_rows(ui, row_h, shown.len(), |ui, range| {
            for f in &shown[range] {
                let sel = self.selected.as_deref() == Some(f.as_str());
                let label = f.strip_prefix("dataraw/").unwrap_or(f);
                if ui.selectable_label(sel, label).clicked() {
                    clicked = Some((*f).clone());
                }
            }
        });
        if let Some(f) = clicked {
            self.selected = Some(f);
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top").show(ctx, |ui| self.top_bar(ui));
        egui::SidePanel::left("files").default_width(320.0).show(ctx, |ui| self.file_list(ui));
        egui::CentralPanel::default().show(ctx, |ui| {
            let (Some(install), Some(sel)) = (&self.install, self.selected.clone()) else {
                ui.centered_and_justified(|ui| {
                    ui.label(if self.install.is_none() { "Open your IG2 install folder." } else { "Select a file." });
                });
                return;
            };
            match self.tab {
                Tab::Textures => self.textures.ui(ui, install, &sel),
                Tab::Models => self.models.ui(ui, frame, install, &sel),
                Tab::Colony => self.colony.ui(ui, install, &sel),
                Tab::Scenarios => self.scn.ui(ui, install, &sel),
                Tab::Animations => self.anis.ui(ui, install, &sel),
                Tab::Messages => self.messages.ui(ui, install, &sel),
            }
        });
    }
}

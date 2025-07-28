use std::collections::{HashMap, HashSet};

use egui_plot::PlotPoints;

use crate::app::loader::CommitReport;

mod loader;

pub struct App {
    config: Config,
    loader: loader::RepositoryLoader,
    data: Vec<CommitReport>,
    show_settings: bool,
    language_filter: HashSet<tokei::LanguageType>,
    language_colors: HashMap<tokei::LanguageType, egui::Color32>,
    show_code: bool,
    show_files: bool,
    show_comments: bool,
    show_blanks: bool,
}
#[derive(Default, Clone, Debug)]
pub struct Config {
    pub depth: usize,
    pub repo_url: String,
    pub repo_branch: String,
}

impl Config {
    fn show(&mut self, ui: &mut egui::Ui) -> bool {
        let mut clicked = false;
        ui.vertical(|ui| {
            egui::Grid::new("config")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Git repo path: ");
                    ui.text_edit_singleline(&mut self.repo_url);
                    ui.end_row();
                    ui.label("Branch: ");
                    ui.text_edit_singleline(&mut self.repo_branch);
                    ui.end_row();
                    ui.label("Depth:");
                    ui.add(egui::widgets::DragValue::new(&mut self.depth));
                });
            if ui.button("process").clicked() {
                clicked = true;
            }
        });
        clicked
    }
}
impl App {
    pub fn new(config: Config, _: &eframe::CreationContext<'_>) -> Self {
        Self {
            config,
            loader: loader::RepositoryLoader::new(),
            data: Vec::new(),
            show_settings: false,
            language_filter: Self::all_languages(),
            language_colors: Self::generate_colors(),
            show_blanks: false,
            show_code: true,
            show_comments: false,
            show_files: false,
        }
    }
    fn all_languages() -> HashSet<tokei::LanguageType> {
        tokei::LanguageType::list()
            .iter()
            .cloned()
            .collect::<HashSet<tokei::LanguageType>>()
    }
    fn generate_colors() -> HashMap<tokei::LanguageType, egui::Color32> {
        Self::all_languages()
            .into_iter()
            .enumerate()
            .map(|(i, l)| {
                let golden_ratio = (5.0_f32.sqrt() - 1.0) / 2.0; // 0.61803398875
                let h = i as f32 * golden_ratio;
                let c: egui::Color32 = egui::ecolor::Hsva::new(h, 0.85, 0.5, 1.0).into();
                (l, c)
            })
            .collect()
    }
    fn collect_data(
        &self,
        getter: impl Fn(&loader::CodeStats) -> usize,
    ) -> Vec<(tokei::LanguageType, Vec<egui_plot::PlotPoint>)> {
        tokei::LanguageType::list()
            .iter()
            .filter_map(|l| {
                if self.language_filter.contains(l) {
                    let loc_data: Vec<_> = self
                        .data
                        .iter()
                        .filter_map(|report| {
                            report.stats.iter().find(|s| s.language == *l).map(|s| {
                                egui_plot::PlotPoint::new(
                                    report.commit_date as f64,
                                    getter(s) as f64,
                                )
                            })
                        })
                        .collect();
                    Some((*l, loc_data))
                } else {
                    None
                }
            })
            .collect()
    }
    fn make_subplot<'d>(
        &self,
        shown_data: &'d [(tokei::LanguageType, Vec<egui_plot::PlotPoint>)],
        plot_ui: &mut egui_plot::PlotUi<'d>,
    ) {
        for (language, loc_data) in shown_data.iter() {
            let color = *self.language_colors.get(language).unwrap();
            let line = egui_plot::Line::new(language.to_string(), PlotPoints::Borrowed(loc_data))
                .style(egui_plot::LineStyle::dashed_dense())
                .color(color)
                .highlight(true);

            plot_ui.line(line);
            let points =
                egui_plot::Points::new(language.to_string(), PlotPoints::Borrowed(loc_data))
                    .radius(4.0)
                    .color(color)
                    .allow_hover(true);
            plot_ui.points(points);
        }
    }
    fn make_plot(&self, _: &egui::Context, ui: &mut egui::Ui) {
        let x_axes = egui_plot::AxisHints::new_x().label("Date").formatter(
            |mark: egui_plot::GridMark, _| {
                use chrono::prelude::*;
                let date = DateTime::from_timestamp(mark.value as i64, 0).unwrap_or_default();
                date.format("%Y-%m-%d").to_string()
            },
        );
        let code_data = self.collect_data(|s| s.code);
        let files_data = self.collect_data(|s| s.files);
        egui_plot::Plot::new("plot")
            .custom_x_axes(vec![x_axes])
            .show(ui, |plot_ui| {
                if self.show_code {
                    self.make_subplot(&code_data, plot_ui);
                }
                if self.show_files {
                    self.make_subplot(&files_data, plot_ui);
                }

            });
    }
    fn show_config_panel(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.heading("Settings");
        });
        ui.separator();
        let mut used_languages: HashSet<tokei::LanguageType> = HashSet::default();
        for d in &self.data {
            for s in &d.stats {
                used_languages.insert(s.language);
            }
        }
        // self.language_filter.clear();
        egui::Grid::new("settings_grid")
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                ui.label(egui::RichText::from("Metrics").underline());
                ui.end_row();
                ui.label("code");
                ui.checkbox(&mut self.show_code, ());
                ui.end_row();
                ui.label("blanks");
                ui.checkbox(&mut self.show_blanks, ());
                ui.end_row();
                ui.label("comments");
                ui.checkbox(&mut self.show_comments, ());
                ui.end_row();
                ui.label("files");
                ui.checkbox(&mut self.show_files, ());
                ui.end_row();
                ui.label(egui::RichText::from("Languages").underline());
                let mut any_selected = !self.language_filter.is_empty();
                let any_selected_copy = any_selected;
                ui.checkbox(&mut any_selected, ());
                if !any_selected && any_selected_copy {
                    self.language_filter.clear();
                } else if any_selected && !any_selected_copy {
                    self.language_filter = Self::all_languages();
                }
                ui.end_row();
                for lang in tokei::LanguageType::list() {
                    if used_languages.contains(lang) {
                        ui.label(lang.to_string());
                        let mut selected = self.language_filter.contains(lang);
                        ui.checkbox(&mut selected, ());
                        if selected {
                            self.language_filter.insert(*lang);
                        } else {
                            self.language_filter.remove(lang);
                        }
                        ui.color_edit_button_srgba(self.language_colors.get_mut(lang).unwrap());
                        ui.end_row();
                    }
                }
            });
    }
    fn show_plot_panel(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        // The central panel the region left after adding TopPanel's and SidePanel's
        ui.heading("Oshmornegar");
        if self.config.show(ui) {
            self.loader.update_config(self.config.clone());
            log::debug!("Start processing");
            self.data.clear();
        }
        if let Some(Ok(loader::LoaderData::CommitReport(r))) = self.loader.try_recv() {
            self.data.push(r);
        }
        ui.ctx().request_repaint();
        self.make_plot(ctx, ui);
    }
}
impl eframe::App for App {
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {}
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:

            egui::MenuBar::new().ui(ui, |ui| {
                // NOTE: no File->Quit on web pages!
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.add_space(16.0);
                ui.toggle_value(&mut self.show_settings, "⚙")
                    .on_hover_text("Settings");
                egui::widgets::global_theme_preference_buttons(ui);
            });
        });
        egui::SidePanel::left("Config")
            .resizable(false)
            .show_animated(ctx, self.show_settings, |ui| self.show_config_panel(ui));
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                powered_by_egui_and_eframe(ui);
                egui::warn_if_debug_build(ui);
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            self.show_plot_panel(ctx, ui);
        });
    }
}

fn powered_by_egui_and_eframe(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label("Powered by ");
        ui.hyperlink_to("egui", "https://github.com/emilk/egui");
        ui.label(", ");
        ui.hyperlink_to(
            "eframe",
            "https://github.com/emilk/egui/tree/master/crates/eframe",
        );
        ui.label(".");
    });
}

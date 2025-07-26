use egui_plot::PlotPoints;
use serde::{Deserialize, Serialize};
use tokei::LanguageType;

use crate::app::loader::CommitReport;

mod loader;

pub struct App {
    config: Config,
    loader: loader::RepositoryLoader,
    data: Vec<CommitReport>,
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
        }
    }

    fn make_plot(&self, ui: &mut egui::Ui) {
        let scala_data: Vec<_> = self
            .data
            .iter()
            .filter_map(|report| {
                report
                    .stats
                    .iter()
                    .find(|s| s.language == LanguageType::Scala)
                    .map(|s| (report.commit_date, s.code))
            })
            .collect();
        let data_points: Vec<_> = scala_data
            .iter()
            .map(|(d, v)| egui_plot::PlotPoint::new(*d as f64, *v as f64))
            .collect();

        let color = egui::Color32::from_rgb(100, 200, 100);

        let line = egui_plot::Line::new("curve", PlotPoints::Borrowed(&data_points))
            .color(color)
            .style(egui_plot::LineStyle::dashed_dense())
            .highlight(true);
        let points = egui_plot::Points::new("points", PlotPoints::Borrowed(&data_points))
            .radius(4.0)
            .color(color)
            .allow_hover(true);

        let x_axes = egui_plot::AxisHints::new_x().label("Date").formatter(
            |mark: egui_plot::GridMark, _| {
                use chrono::prelude::*;
                let date = DateTime::from_timestamp(mark.value as i64, 0).unwrap_or_default();
                date.format("%Y-%m-%d").to_string()
            },
        );
        egui_plot::Plot::new("code")
            .custom_x_axes(vec![x_axes])
            .show(ui, |plot_ui| {
                plot_ui.line(line);
                plot_ui.points(points);
            });
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
                egui::widgets::global_theme_preference_buttons(ui);
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
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
            self.make_plot(ui);

            ui.add(egui::github_link_file!(
                "https://github.com/emilk/eframe_template/blob/main/",
                "Source code."
            ));

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                powered_by_egui_and_eframe(ui);
                egui::warn_if_debug_build(ui);
            });
        });
    }
}

fn powered_by_egui_and_eframe(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label("Powered by ");
        ui.hyperlink_to("egui", "https://github.com/emilk/egui");
        ui.label(" and ");
        ui.hyperlink_to(
            "eframe",
            "https://github.com/emilk/egui/tree/master/crates/eframe",
        );
        ui.label(".");
    });
}

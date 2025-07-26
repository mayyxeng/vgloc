use clap::Parser;

use crate::app::Config;
mod app;
#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct CliArgs {
    /// URL or path to the repo
    #[arg(short, long)]
    path: Option<String>,
    /// Number of walk backs
    #[arg(short, long, default_value_t = 0)]
    depth: usize,
    /// Branch name
    #[arg(short, long)]
    branch: Option<String>,
}
fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let args = CliArgs::parse();
    let config = Config {
        depth: args.depth,
        repo_url: args.path.unwrap_or_default(),
        repo_branch: args.branch.unwrap_or_default(),
    };
    log::debug!("Parsed config: {config:?}");
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1920.0, 1080.])
            .with_min_inner_size([1900.0, 1060.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Oshmornegar",
        native_options,
        Box::new(|cc| Ok(Box::new(app::App::new(config, cc)))),
    )
}

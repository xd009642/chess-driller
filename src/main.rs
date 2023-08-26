use std::env;
use tracing::info;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::{Layer, Registry};

fn main() -> anyhow::Result<()> {
    let filter = match env::var("RUST_LOG") {
        Ok(_) => EnvFilter::from_env("RUST_LOG"),
        _ => EnvFilter::new("chess_driller=info"),
    };

    let fmt = tracing_subscriber::fmt::Layer::default();

    let subscriber = filter.and_then(fmt).with_subscriber(Registry::default());

    tracing::subscriber::set_global_default(subscriber)?;
    info!("Starting chess driller");

    let options = eframe::NativeOptions {
        drag_and_drop_support: true,
        initial_window_size: Some([800.0, 800.0].into()),
        ..Default::default()
    };
    eframe::run_native(
        "chess-driller",
        options,
        Box::new(|_cc| Box::new(App::new())),
    )
    chess_driller::run()
}

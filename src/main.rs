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

    chess_driller::run()
}

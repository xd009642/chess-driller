// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::env;
use tracing::info;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::{Layer, Registry};
use std::sync::Mutex;


fn main() {
    let filter = match env::var("RUST_LOG") {
        Ok(_) => EnvFilter::from_env("RUST_LOG"),
        _ => EnvFilter::new("chess_driller=info"),
    };

    let fmt = tracing_subscriber::fmt::Layer::default();

    let subscriber = filter.and_then(fmt).with_subscriber(Registry::default());

    tracing::subscriber::set_global_default(subscriber).unwrap();
    info!("Starting chess driller");

    chess_driller::launch();
}

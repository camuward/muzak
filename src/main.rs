// On Windows do NOT show a console window when opening the app
#![cfg_attr(
    all(not(test), not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::sync::LazyLock;

use tracing::level_filters::LevelFilter;
use tracing_subscriber::{EnvFilter, prelude::*};

mod devices;
mod library;
mod media;
mod playback;
mod services;
mod settings;
mod ui;
mod util;

static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .build()
        .unwrap()
});

fn main() -> anyhow::Result<()> {
    let reg = tracing_subscriber::registry();

    #[cfg(feature = "console")]
    let reg = reg.with(console_subscriber::spawn());
    reg.with({
        const PREFERRED: &str = "HUMMINGBIRD_LOG";
        tracing_subscriber::fmt::layer().with_filter(
            EnvFilter::builder()
                .with_env_var(std::env::var_os(PREFERRED).map_or("RUST_LOG", |_| PREFERRED))
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
    })
    .init();

    tracing::info!("Starting application");

    crate::ui::app::run()
}

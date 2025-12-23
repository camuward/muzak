// On Windows do NOT show a console window when opening the app
#![cfg_attr(
    all(not(test), not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::sync::LazyLock;

use tracing_subscriber::{fmt::format::FmtSpan, prelude::*};

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

    let env = tracing_subscriber::EnvFilter::builder().parse(
        ["HUMMINGBIRD_LOG", "RUST_LOG"] // prefer Hummingbird-specific variable
            .iter() // find the first one that's set at all
            .find_map(|key| std::env::var(key).ok()) // even if it's empty
            .filter(|s| !s.is_empty()) // NOW we can check is_empty and use default
            .unwrap_or_else(|| "info,blade_graphics=warn,symphonia=warn,zbus=warn".to_owned()),
    )?; // inform user they have a malformed filter

    reg.with(
        tracing_subscriber::fmt::layer()
            .with_thread_names(true) // nice to have until we replace with tasks
            .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE) // async can be noisy
            .with_timer(tracing_subscriber::fmt::time::uptime()) // date's useless
            .with_filter(env),
    )
    .init();

    tracing::info!("Starting application");

    crate::ui::app::run()
}

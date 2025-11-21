// On Windows do NOT show a console window when opening the app
#![cfg_attr(
    all(not(test), not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::sync::LazyLock;

use tracing_subscriber::prelude::*;

mod devices;
mod library;
mod media;
mod playback;
mod services;
mod settings;
mod ui;
mod util;

const VERSION_STRING: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    env!("HUMMINGBIRD_VERSION_SUFFIX")
);

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

    reg.with(tracing_subscriber::fmt::layer()).init();

    tracing::info!("version {VERSION_STRING}");

    crate::ui::app::run()
}

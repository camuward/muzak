use std::{
    fs::File,
    path::Path,
    sync::{Arc, LazyLock},
};

// use tokio rwlock because it is write-preferring
use tokio::sync::RwLock;
use tracing::info;

use crate::media::traits::{MediaProvider, MediaProviderFeatures, MediaStream};

type LookupTableInnerType = Arc<RwLock<Vec<Box<dyn MediaProvider>>>>;

pub static LOOKUP_TABLE: LazyLock<LookupTableInnerType> =
    LazyLock::new(|| Arc::new(RwLock::new(Vec::new())));

pub fn add_provider(provider: Box<dyn MediaProvider>) {
    info!(
        "Attempting to register media provider \"{}\"",
        provider.name()
    );

    let mut write = LOOKUP_TABLE.blocking_write();
    write.push(provider);
}

#[allow(clippy::borrowed_box)]
fn provider_can_read(
    path: &Path,
    required_features: MediaProviderFeatures,
    provider: &Box<dyn MediaProvider>,
) -> anyhow::Result<bool> {
    let mime = infer::get_from_path(path);
    let mut found = false;

    if let Some(mime) = mime?
        && provider
            .supported_mime_types()
            .iter()
            .any(|t| *t == mime.mime_type())
    {
        found = true;
    }

    if !found
        && let Some(ext) = path.extension().and_then(|v| v.to_str())
        && provider.supported_extensions().contains(&ext)
    {
        found = true;
    }

    Ok(found && provider.supported_features() & required_features == required_features)
}

pub fn can_be_read(path: &Path, required_features: MediaProviderFeatures) -> anyhow::Result<bool> {
    let read = LOOKUP_TABLE.blocking_read();
    for provider in read.iter() {
        if provider_can_read(path, required_features, provider)? {
            return Ok(true);
        }
    }

    Ok(false)
}

pub fn try_open_media(
    path: &Path,
    required_features: MediaProviderFeatures,
) -> anyhow::Result<Option<Box<dyn MediaStream>>> {
    let read = LOOKUP_TABLE.blocking_read();
    for provider in read.iter() {
        if provider_can_read(path, required_features, provider)? {
            let file = File::open(path)?;
            return Ok(Some(provider.open(file, path.extension())?));
        }
    }

    Ok(None)
}

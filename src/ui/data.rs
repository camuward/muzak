use std::{
    hash::Hasher,
    io::Cursor,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
};

use futures::TryFutureExt as _;
use gpui::{App, Entity, RenderImage, Task};
use image::{Frame, ImageReader, imageops::thumbnail};
use moka::sync::Cache;
use rustc_hash::FxHasher;
use smallvec::smallvec;
use tracing::{debug, error, trace_span, warn};

use crate::{
    media::{builtin::symphonia::SymphoniaProvider, metadata::Metadata, traits::MediaProvider},
    playback::queue::{DataSource, QueueItemUIData},
    util::rgb_to_bgr,
};

static ALBUM_CACHE: LazyLock<Cache<u64, Arc<RenderImage>>> = LazyLock::new(|| Cache::new(30));

#[tracing::instrument(level = "trace", skip(data))]
fn decode_image(data: Box<[u8]>, thumb: bool) -> anyhow::Result<Arc<RenderImage>> {
    let mut image = ImageReader::new(Cursor::new(data))
        .with_guessed_format()?
        .decode()?
        .into_rgba8();

    rgb_to_bgr(&mut image);

    let frame = if thumb {
        Frame::new(thumbnail(&image, 80, 80))
    } else {
        Frame::new(image)
    };

    Ok(Arc::new(RenderImage::new(smallvec![frame])))
}

#[tracing::instrument(level = "trace")]
fn read_metadata(path: &Path) -> anyhow::Result<QueueItemUIData> {
    let file = std::fs::File::open(path)?;

    // TODO: Switch to a different media provider based on the file
    let mut stream = SymphoniaProvider.open(file, None)?;
    stream.start_playback()?;

    let Metadata { name, artist, .. } = stream.read_metadata()?;
    let mut ui_data = QueueItemUIData {
        name: name.as_ref().map(Into::into),
        artist_name: artist.as_ref().map(Into::into),
        source: DataSource::Metadata,
        image: None,
    };

    match stream.read_image() {
        Err(err) => warn!(path = %path.display(), ?err, "Album art unavailable: {err}"),
        Ok(None) => debug!(path = %path.display(), "No image provided"),
        Ok(Some(data)) => {
            let _g = trace_span!("retrieving album art", path = %path.display()).entered();
            let hash = {
                let mut hasher = FxHasher::default();
                hasher.write(&data);
                hasher.finish()
            };
            if let Ok(img) = ALBUM_CACHE.try_get_with(hash, || {
                debug!(%hash, "album art cache miss, decoding image");
                decode_image(data, true).inspect_err(|err| {
                    warn!(?err, "Failed to decode album art: {err}");
                })
            }) {
                ui_data.image.replace(img);
            }
        }
    }

    Ok(ui_data)
}

pub trait Decode {
    fn decode_image(
        &self,
        data: Box<[u8]>,
        thumb: bool,
        entity: Entity<Option<Arc<RenderImage>>>,
    ) -> Task<()>;
    fn read_metadata(&self, path: PathBuf, entity: Entity<Option<QueueItemUIData>>) -> Task<()>;
}

impl Decode for App {
    fn decode_image(
        &self,
        data: Box<[u8]>,
        thumb: bool,
        entity: Entity<Option<Arc<RenderImage>>>,
    ) -> Task<()> {
        self.spawn(async move |cx| {
            let task = crate::RUNTIME.spawn_blocking(move || decode_image(data, thumb));
            match task.err_into().await.flatten() {
                Err(err) => error!(?err, "Failed to decode image: {err}"),
                Ok(img) => entity
                    .update(cx, |m, cx| {
                        *m = Some(img);
                        cx.notify();
                    })
                    .expect("Failed to update RenderImage entity"),
            }
        })
    }

    fn read_metadata(&self, path: PathBuf, entity: Entity<Option<QueueItemUIData>>) -> Task<()> {
        self.spawn(async move |cx| {
            let span = trace_span!("read_metadata_outer", path = %path.display());
            let task = crate::RUNTIME.spawn_blocking(move || read_metadata(&path));
            match task.err_into().await.flatten() {
                Err(err) => error!(parent: span, ?err, "Failed to read metadata: {err}"),
                Ok(metadata) => entity
                    .update(cx, |m, cx| {
                        *m = Some(metadata);
                        cx.notify();
                    })
                    .expect("Failed to update metadata entity"),
            }
        })
    }
}

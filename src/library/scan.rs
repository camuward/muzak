#![allow(
    clippy::redundant_closure_for_method_calls,
    clippy::needless_pass_by_ref_mut,
    clippy::semicolon_if_nothing_returned,
    dead_code
)]

use std::{
    future::ready,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::SystemTime,
};

use futures::{StreamExt, TryFutureExt, future::OptionFuture};
use globwalk::GlobWalkerBuilder;
use gpui::{App, Global};
use rustc_hash::FxHashMap;
use sqlx::SqlitePool;
use tokio::fs::{self, File};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::mpsc::{self, Receiver, Sender, UnboundedSender};
use tokio_stream::StreamExt as _;
use tracing::{debug, error, info, warn};

use crate::{
    media::{
        builtin::symphonia::SymphoniaProvider,
        metadata::Metadata,
        traits::{MediaPlugin, MediaProvider},
    },
    settings::scan::ScanSettings,
    ui::{app::get_dirs, models::Models},
};

/// The version of the scanning process. If this version number is incremented, a re-scan of all
/// files will be forced (see [`ScanCommand::ForceScan`]).
const SCAN_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanEvent {
    Cleaning,
    DiscoverProgress(u64),
    ScanProgress { current: u64, total: u64 },
    ScanCompleteWatching,
    ScanCompleteIdle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanCommand {
    Scan,
    /// A force-scan is different to a regular scan in that it will ignore all previous data and
    /// instead re-scan all tracks and re-create all album information. This is necessary when the
    /// database schema has been changed, or a bug has been fixed with in the scanning proccess,
    /// and is usually triggered by the scan version changing (see [`SCAN_VERSION`]).
    ForceScan,
}

#[derive(Clone)]
pub struct ScanInterface {
    cmd_tx: Sender<ScanCommand>,
}

impl ScanInterface {
    pub fn scan(&self) {
        self.cmd_tx
            .blocking_send(ScanCommand::Scan)
            .expect("could not send scan start command");
    }

    pub fn force_scan(&self) {
        self.cmd_tx
            .blocking_send(ScanCommand::ForceScan)
            .expect("could not send force re-scan start command");
    }
}

impl Global for ScanInterface {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanState {
    Idle,
    Cleanup,
    Discovering,
    Scanning,
}

type ScanRecord = dashmap::DashMap<PathBuf, u64>;

pub struct Scanner {
    task: tokio::task::JoinHandle<()>,
}

impl Global for Scanner {}

static TOTAL_TRACKS: AtomicU64 = AtomicU64::new(0);
static TOTAL_DUR_MS: AtomicU64 = AtomicU64::new(0);

impl Scanner {
    pub fn new(cx: &mut App, pool: SqlitePool, settings: ScanSettings) -> ScanInterface {
        let (cmd_tx, commands_rx) = mpsc::channel(10);
        let (events_tx, events_rx) = mpsc::unbounded_channel();

        let handle = ScanInterface { cmd_tx };
        let state_model = cx.global::<Models>().scan_state.clone();
        cx.spawn(async move |cx| {
            tokio_stream::wrappers::UnboundedReceiverStream::new(events_rx)
                // .map(futures::future::ready)
                .chunks_timeout(10, std::time::Duration::from_millis(100))
                .for_each(|chunk| {
                    let Some(&event) = chunk.last() else {
                        return ready(());
                    };
                    state_model
                        .update(cx, |m, cx| {
                            *m = event;
                            cx.notify()
                        })
                        .expect("failed to update scan state model");
                    ready(())
                })
                .await;
        })
        .detach();

        cx.set_global(Scanner {
            task: crate::RUNTIME.spawn(async move {
                let scan_record_path = get_dirs().data_dir().join("scan_record");
                _ = fs::remove_file(scan_record_path.with_extension("json")).await;

                let scan_record: ScanRecord = self
                    .read_scan_record(&scan_record_path)
                    .inspect_err(|err| {
                        error!(?err, "could not read scan record: {err}");
                        error!("scanning will be slow until the scan record is rebuilt");
                    })
                    .unwrap_or_default();

                loop {
                    self.read_commands();

                    // TODO: start file watcher to update db automatically when files are added or removed
                    match self.scan_state {
                        ScanState::Idle => {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        }
                        ScanState::Cleanup => {
                            futures::stream::iter(std::mem::take(&mut self.scan_record))
                                .for_each_concurrent(None, async |(path, ts)| {
                                    if tokio::fs::try_exists(&path)
                                        .await
                                        .is_ok_and(|exists| exists)
                                    {
                                        self.scan_record.insert(path, ts);
                                        return;
                                    }

                                    debug!("track deleted or moved: {}", path.display());
                                    let result = sqlx::query(include_str!(
                                        "../../queries/scan/delete_track.sql"
                                    ))
                                    .bind(path.to_str())
                                    .execute(&self.pool)
                                    .await;

                                    if let Err(e) = result {
                                        error!("Database error while deleting track: {:?}", e);
                                        self.scan_record.insert(path, ts);
                                    }
                                })
                                .await;
                            self.scan_state = ScanState::Discovering;
                        }
                        ScanState::Discovering => {
                            self.discover();
                        }
                        ScanState::Scanning => 'scan: {
                            let Some(path) = self.to_process.pop() else {
                                info!("Scan complete, writing scan record and stopping");
                                self.write_scan_record().unwrap_or_else(|err| {
                            error!(?err, "Could not write scan record: {err}");
                            error!(
                                "Scan record will not be saved, this may cause rescans on restart"
                            );
                        });

                                self.scan_state = ScanState::Idle;
                                self.event_tx
                                    .send(ScanEvent::ScanCompleteIdle)
                                    .expect("could not send scan event");
                                break 'scan;
                            };

                            let Some(metadata) = self.get_file_info(&path) else {
                                warn!("Could not read metadata for {}", path.display());
                                break 'scan;
                            };

                            if let Err(err) =
                                crate::RUNTIME.block_on(self.record_file_info(&path, metadata))
                            {
                                error!("Failed to update metadata for {}: {err}", path.display());
                            }

                            self.scanned += 1;

                            if self.scanned.is_multiple_of(5) {
                                self.event_tx
                                    .send(ScanEvent::ScanProgress {
                                        current: self.scanned,
                                        total: self.discovered_total,
                                    })
                                    .expect("could not send scan event");
                            }
                        }
                    }
                }
            }),
        });

        handle
    }

    fn read_commands(&mut self) {
        while let Ok(command) = self.command_rx.try_recv() {
            match command {
                ScanCommand::Scan => {
                    if self.scan_state == ScanState::Idle {
                        self.discovered = self.scan_settings.paths.clone();
                        self.scan_state = ScanState::Cleanup;
                        self.scanned = 0;
                        self.discovered_total = 0;
                        self.discovered = self.scan_settings.paths.clone();
                        self.visited.clear();
                        self.to_process.clear();
                        self.is_force = false;

                        self.event_tx
                            .send(ScanEvent::Cleaning)
                            .expect("could not send scan event");
                    }
                }
                ScanCommand::ForceScan => {
                    if self.scan_state == ScanState::Idle {
                        self.discovered = self.scan_settings.paths.clone();
                        self.scan_state = ScanState::Cleanup;
                        self.scanned = 0;
                        self.discovered_total = 0;
                        self.discovered = self.scan_settings.paths.clone();
                        self.visited.clear();
                        self.to_process.clear();

                        self.is_force = true;
                        self.forced_albums.clear();

                        self.scan_record.clear();

                        self.event_tx
                            .send(ScanEvent::Cleaning)
                            .expect("could not send scan event");
                    }
                }
            }
        }

        if self.scan_state == ScanState::Discovering {
            self.discover();
        } else if self.scan_state == ScanState::Scanning {
            self.scan();
        } else {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    fn file_is_scannable(&mut self, path: &PathBuf) -> bool {
        let timestamp = match fs::metadata(path) {
            Ok(metadata) => metadata
                .modified()
                .unwrap()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            Err(_) => return false,
        };
        self.provider_table
            .iter()
            .any(|(exts, _)| file_is_scannable_with_provider(path, exts))
            && self
                .scan_record
                .get(path)
                .is_none_or(|ts| ts.value() != &timestamp)
            && {
                self.scan_record.insert(path.clone(), timestamp);
                true
            }
    }

    fn discover(&mut self) {
        if self.discovered.is_empty() {
            self.scan_state = ScanState::Scanning;
            return;
        }

        let path = self.discovered.pop().unwrap();

        if self.visited.contains(&path) {
            return;
        }

        let paths = fs::read_dir(&path).unwrap();

        for paths in paths {
            // TODO: handle errors
            // this might be slower than just reading the path directly but this prevents loops
            let path = paths.unwrap().path().canonicalize().unwrap();
            if path.is_dir() {
                self.discovered.push(path);
            } else if self.file_is_scannable(&path) {
                self.to_process.push(path);

                self.discovered_total += 1;

                if self.discovered_total.is_multiple_of(20) {
                    self.event_tx
                        .send(ScanEvent::DiscoverProgress(self.discovered_total))
                        .expect("could not send scan event");
                }
            }
        }

        self.visited.push(path);
    }

    fn get_file_info(&mut self, path: &PathBuf) -> Option<FileInformation> {
        for (exts, provider) in &mut self.provider_table {
            if file_is_scannable_with_provider(path, exts)
                && let Ok(mut metadata) = scan_file_with_provider(path, provider)
            {
                if metadata.2.is_none() {
                    metadata.2 = scan_path_for_album_art(path);
                }

                return Some(metadata);
            }
        }

        None
    }
}

async fn read_scan_record(path: &Path) -> io::Result<ScanRecord> {
    let mut record = BufReader::new(File::open(path).await?);
    let mut magic = [0; 2];
    record.read_exact(&mut magic).await?;
    let magic = u16::from_le_bytes(magic);
    if magic == SCAN_VERSION {
        Ok(serde_json::from_reader::<_, FxHashMap<_, _>>(record)?
            .into_iter()
            .collect())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("expected scan version {SCAN_VERSION}, got {magic}"),
        ))
    }
}

async fn write_scan_record(path: &Path, record: ScanRecord) -> io::Result<()> {
    let mut file = BufWriter::new(File::create(path).await?);
    file.write_all(&SCAN_VERSION.to_le_bytes()).await?;
    serde_json::to_writer(
        &mut file,
        record.iter()
            .map(|e| (e.key().clone(), *e.value()))
            .collect::<FxHashMap<_, _>>(),
    )?;
    info!("Scan record written to {}", self.scan_record_path.display());
    file.into_inner()?.sync_data()
}

async fn decode_image(encoded: Box<[u8]>) -> anyhow::Result<Option<(Box<[u8]>, Vec<u8>)>> {
    use std::io::Cursor;

    use image::codecs::jpeg::JpegEncoder;
    use image::imageops::{self, FilterType::Lanczos3};
    use image::{GenericImageView, ImageFormat, ImageReader};

    let join_handle = crate::RUNTIME.spawn_blocking(move || {
        // if there is a decode error, just ignore it and pretend there is no image
        let Ok(decoded) = ImageReader::new(Cursor::new(&*encoded))
            .with_guessed_format()?
            .decode()
            .inspect_err(|err| warn!(?err, "Could not decode album art image: {err}"))
        else {
            return Ok(None);
        };

        let mut thumb = Vec::with_capacity(19722);
        imageops::thumbnail(&decoded, 70, 70)
            .write_to(&mut Cursor::new(&mut thumb), ImageFormat::Bmp)?;

        // if full image is already small enough, return it as-is, otherwise resize and recompress
        if let (..=1024, ..=1024) = decoded.dimensions() {
            return Ok(Some((encoded, thumb)));
        }

        info!(
            "Resizing album art image from {}x{} to 1024x1024",
            decoded.width(),
            decoded.height()
        );
        let mut resized = vec![];
        imageops::resize(&decoded.to_rgb8(), 1024, 1024, Lanczos3)
            .write_with_encoder(JpegEncoder::new_with_quality(Cursor::new(&mut resized), 70))?;

        Ok(Some((resized.into_boxed_slice(), thumb)))
    });

    join_handle.await?
}

fn build_provider_table() -> Vec<(&'static [&'static str], Box<dyn MediaProvider>)> {
    // TODO: dynamic plugin loading
    vec![(
        SymphoniaProvider::SUPPORTED_EXTENSIONS,
        Box::new(SymphoniaProvider::default()),
    )]
}

fn file_is_scannable_with_provider(path: &Path, exts: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| exts.contains(&e))
}

type FileInformation = (Metadata, u64, Option<Box<[u8]>>);

fn scan_file_with_provider(
    path: &PathBuf,
    provider: &mut Box<dyn MediaProvider>,
) -> Result<FileInformation, ()> {
    let src = std::fs::File::open(path).map_err(|_| ())?;
    provider.open(src, None).map_err(|_| ())?;
    provider.start_playback().map_err(|_| ())?;
    let metadata = provider.read_metadata().cloned().map_err(|_| ())?;
    let image = provider.read_image().map_err(|_| ())?;
    let len = provider.duration_secs().map_err(|_| ())?;
    provider.close().map_err(|_| ())?;
    Ok((metadata, len, image))
}

// Returns the first image (cover/front/folder.jpeg/png/jpeg) in the track's containing folder
// Album art can be named anything, but this pattern is convention and the least likely to return a false positive
fn scan_path_for_album_art(path: &Path) -> Option<Box<[u8]>> {
    // let glob = GlobWalkerBuilder::from_patterns(
    //     path.parent().unwrap(),
    //     &["{folder,cover,front}.{jpg,jpeg,png}"],
    // )
    // .case_insensitive(true)
    // .max_depth(1)
    // .build()
    // .expect("Failed to build album art glob")
    // .filter_map(|e| e.ok());

    // for entry in glob {
    //     if let Ok(bytes) = fs::read(entry.path()) {
    //         return Some(bytes.into_boxed_slice());
    //     }
    // }
    None
}

mod db {
    use super::*;

    pub async fn record_file_info(
        pool: &SqlitePool,
        path: &Path,
        info: FileInformation,
        force: &Encountered,
    ) -> anyhow::Result<()> {
        let (meta, len, image) = info;
        let Metadata { artist, name, .. } = &meta;
        debug!("Adding/updating record for {artist:?} - {name:?}");

        let artist_id = get_or_insert_artist(pool, &meta).await?;
        let album_id = get_or_insert_album(pool, &meta, artist_id, image, force).await?;
        get_or_insert_track(pool, &meta, album_id, path, len).await
    }

    async fn get_or_insert_artist(pool: &SqlitePool, m: &Metadata) -> sqlx::Result<Option<i64>> {
        let Some(artist) = m.album_artist.as_deref().or(m.artist.as_deref()) else {
            return Ok(None);
        };
        let sortable = m.artist_sort.as_deref().unwrap_or(artist);
        sqlx::query_as(include_str!("../../queries/scan/create_artist.sql"))
            .bind(artist)
            .bind(sortable)
            .fetch_one(pool)
            .or_else(|_| {
                sqlx::query_as(include_str!("../../queries/scan/get_artist_id.sql"))
                    .bind(artist)
                    .fetch_one(pool)
            })
            .map_ok(|(id,)| id)
            .await
    }

    async fn get_or_insert_album(
        pool: &SqlitePool,
        metadata: &Metadata,
        artist_id: Option<i64>,
        image: Option<Box<[u8]>>,
        force: &Encountered,
    ) -> anyhow::Result<Option<i64>> {
        let Some(album) = &metadata.album else {
            return Ok(None);
        };
        let mbid = metadata.mbid_album.as_deref().unwrap_or("none");
        let album_id: sqlx::Result<(i64,)> =
            sqlx::query_as(include_str!("../../queries/scan/get_album_id.sql"))
                .bind(album)
                .bind(mbid)
                .fetch_one(pool)
                .await;

        let should_insert = match &album_id {
            Err(sqlx::Error::RowNotFound) => true,
            Ok((id,)) => force.must_insert(id),
            Err(_) => force.is_force(),
        };

        if should_insert {
            let (resized_image, thumb) = OptionFuture::from(image.map(decode_image))
                .await
                .transpose()?
                .flatten()
                .unzip();

            let (id,) = sqlx::query_as(include_str!("../../queries/scan/create_album.sql"))
                .bind(album)
                .bind(metadata.sort_album.as_ref().unwrap_or(album))
                .bind(artist_id)
                .bind(resized_image)
                .bind(thumb)
                .bind(metadata.date)
                .bind(metadata.year)
                .bind(&metadata.label)
                .bind(&metadata.catalog)
                .bind(&metadata.isrc)
                .bind(mbid)
                .fetch_one(pool)
                .await?;

            Ok(Some(id))
        } else {
            let (id,) = album_id?;
            Ok(Some(id))
        }
    }

    async fn get_or_insert_track(
        pool: &SqlitePool,
        metadata: &Metadata,
        album_id: Option<i64>,
        path: &Path,
        length: u64,
    ) -> anyhow::Result<()> {
        if album_id.is_none() {
            return Ok(());
        }

        let disc_num = metadata.disc_current.map(|v| v as i64).unwrap_or(-1);
        let find_path: sqlx::Result<(String,)> =
            sqlx::query_as(include_str!("../../queries/scan/get_album_path.sql"))
                .bind(album_id)
                .bind(disc_num)
                .fetch_one(pool)
                .await;

        let parent = path.parent().unwrap();

        match find_path {
            Ok(path) => {
                if path.0.as_str() != parent.as_os_str() {
                    return Ok(());
                }
            }
            Err(sqlx::Error::RowNotFound) => {
                sqlx::query(include_str!("../../queries/scan/create_album_path.sql"))
                    .bind(album_id)
                    .bind(parent.to_str())
                    .bind(disc_num)
                    .execute(pool)
                    .await?;
            }
            Err(e) => return Err(e.into()),
        }

        let name = metadata
            .name
            .clone()
            .or_else(|| {
                path.file_name()
                    .and_then(|x| x.to_str())
                    .map(|x| x.to_string())
            })
            .ok_or_else(|| anyhow::anyhow!("failed to retrieve filename"))?;

        let result: sqlx::Result<(i64,)> =
            sqlx::query_as(include_str!("../../queries/scan/create_track.sql"))
                .bind(&name)
                .bind(&name)
                .bind(album_id)
                .bind(metadata.track_current.map(|x| x as i32))
                .bind(metadata.disc_current.map(|x| x as i32))
                .bind(length as i32)
                .bind(path.to_str())
                .bind(&metadata.genre)
                .bind(&metadata.artist)
                .bind(parent.to_str())
                .fetch_one(pool)
                .await;

        match result {
            Ok(_) => Ok(()),
            Err(sqlx::Error::RowNotFound) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

/// Keeps track of encountered albums during a force-scan to determine whether an album needs to be
/// re-inserted.
struct Encountered {
    is_force: AtomicBool,
    filter: fastbloom::AtomicBloomFilter<rustc_hash::FxBuildHasher>,
}

impl Encountered {
    fn new() -> Self {
        Self {
            is_force: false.into(),
            filter: fastbloom::AtomicBloomFilter::with_num_bits(1024)
                .hasher(rustc_hash::FxBuildHasher)
                .hashes(4),
        }
    }

    fn force(&self) {
        self.is_force.store(true, Ordering::Release);
        self.filter.clear();
    }

    fn is_force(&self) -> bool {
        self.is_force.load(Ordering::Relaxed)
    }

    fn must_insert(&self, album_id: &i64) -> bool {
        self.is_force() && !self.filter.insert(album_id)
    }
}

use std::{collections::HashMap, fs, path::PathBuf};

use gpui::{Pixels, px};
use serde::{Deserialize, Serialize};

use crate::{library::db::LikedTrackSortMethod, ui::models::CurrentTrack};

pub const DEFAULT_SIDEBAR_WIDTH: Pixels = px(225.0);
pub const DEFAULT_QUEUE_WIDTH: Pixels = px(275.0);
pub const DEFAULT_SPLIT_FRACTION: Pixels = px(0.50);
pub const DEFAULT_LYRICS_FRACTION: Pixels = px(0.35);

fn default_sidebar_width() -> f32 {
    f32::from(DEFAULT_SIDEBAR_WIDTH)
}

fn default_queue_width() -> f32 {
    f32::from(DEFAULT_QUEUE_WIDTH)
}

fn default_split_fraction() -> f32 {
    f32::from(DEFAULT_SPLIT_FRACTION)
}

fn default_volume() -> f64 {
    1.0
}

fn default_table_settings() -> HashMap<String, TableSettings> {
    HashMap::new()
}

fn default_table_view_mode() -> TableViewModeSetting {
    TableViewModeSetting::List
}

fn default_liked_tracks_sort_method() -> LikedTrackSortMethod {
    LikedTrackSortMethod::ReleaseOrder
}

fn default_lyrics_fraction() -> f32 {
    f32::from(DEFAULT_LYRICS_FRACTION)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TableViewModeSetting {
    #[default]
    List,
    Grid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TableSettings {
    #[serde(default)]
    pub column_widths: HashMap<String, f32>,
    /// Visible columns in display order. Absent columns are treated as hidden.
    #[serde(default)]
    pub column_order: Vec<String>,
    /// Legacy field used to migrate old settings files.
    #[serde(default, skip_serializing)]
    pub hidden_columns: Vec<String>,
    #[serde(default = "default_table_view_mode")]
    pub view_mode: TableViewModeSetting,
}

/// Data to store while quitting the app
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageData {
    pub current_track: Option<CurrentTrack>,
    #[serde(default = "default_volume")]
    pub volume: f64,
    /// Width of the library sidebar in pixels
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: f32,
    /// Width of the queue panel in pixels
    #[serde(default = "default_queue_width")]
    pub queue_width: f32,
    /// Fraction (0..1) of the two-column split pane width
    #[serde(default = "default_split_fraction")]
    pub split_fraction: f32,
    #[serde(default = "default_table_settings")]
    pub table_settings: HashMap<String, TableSettings>,
    #[serde(default = "default_liked_tracks_sort_method")]
    pub liked_tracks_sort_method: LikedTrackSortMethod,
    #[serde(default)]
    pub sidebar_collapsed: bool,
    /// Fraction (0..1) of the lyrics panel height
    #[serde(default = "default_lyrics_fraction")]
    pub lyrics_fraction: f32,
}

impl StorageData {
    pub fn sidebar_width(&self) -> Pixels {
        px(self.sidebar_width)
    }

    pub fn queue_width(&self) -> Pixels {
        px(self.queue_width)
    }

    pub fn split_fraction(&self) -> Pixels {
        px(self.split_fraction)
    }

    pub fn lyrics_fraction(&self) -> Pixels {
        px(self.lyrics_fraction)
    }
}

impl Default for StorageData {
    fn default() -> Self {
        Self {
            current_track: None,
            volume: default_volume(),
            sidebar_width: f32::from(DEFAULT_SIDEBAR_WIDTH),
            queue_width: f32::from(DEFAULT_QUEUE_WIDTH),
            split_fraction: f32::from(DEFAULT_SPLIT_FRACTION),
            table_settings: HashMap::new(),
            liked_tracks_sort_method: default_liked_tracks_sort_method(),
            sidebar_collapsed: false,
            lyrics_fraction: f32::from(DEFAULT_LYRICS_FRACTION),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Storage {
    /// File path to store data
    path: PathBuf,
}

impl Storage {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Save `StorageData` on file system
    pub fn save(&self, data: &StorageData) {
        // save into file
        let result = fs::File::create(self.path.clone())
            .and_then(|file| serde_json::to_writer(file, &data).map_err(|e| e.into()));
        // ignore error, but log it
        if let Err(e) = result {
            tracing::warn!("could not save `AppState` {:?}", e);
        };
    }

    /// Load `StorageData` from storage or use `StorageData::default` in case of any errors
    pub fn load_or_default(&self) -> StorageData {
        std::fs::File::open(self.path.clone())
            .and_then(|file| {
                serde_json::from_reader(file)
                    .map_err(|e| e.into())
                    .map(|data: StorageData| match &data.current_track {
                        // validate whether path still exists
                        Some(current_track) if !current_track.get_path().exists() => StorageData {
                            current_track: None,
                            // Preserve other settings when invalidating current_track
                            ..data
                        },
                        _ => data,
                    })
            })
            .unwrap_or_default()
    }
}

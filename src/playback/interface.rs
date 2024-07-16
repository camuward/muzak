use std::{
    sync::mpsc::{Receiver, Sender},
    time::Duration,
};

use gpui::{AppContext, Model};
use image::RgbaImage;

use crate::{media::metadata::Metadata, ui::models::Models};

use super::{
    events::{PlaybackCommand, PlaybackEvent},
    thread::PlaybackState,
};

/// The PlaybackInterface trait defines the method used to create the struct that will be used to
/// communicate between the playback thread and the main thread.
pub trait PlaybackInterface {
    fn new(commands_tx: Sender<PlaybackCommand>, events_rx: Receiver<PlaybackEvent>) -> Self;
}

/// The playback interface struct that will be used to communicate between the playback thread and
/// the main thread. This implementation takes advantage of the GPUI Global trait to allow any
/// function (so long as it is running on the main thread) to send commands to the playback thread.
///
/// This interface takes advantage of GPUI's asynchronous runtime to read messages without blocking
/// rendering. Messages are read at quickest every 50ms, however the runtime may choose to run the
/// function that reads events less frequently, depending on the current workload. Because of this,
/// event handling should not perform any heavy operations, which should be instead sent to the
/// data thread for any required additional processing.
pub struct GPUIPlaybackInterface {
    commands_tx: Sender<PlaybackCommand>,
    events_rx: Option<Receiver<PlaybackEvent>>,
}

impl gpui::Global for GPUIPlaybackInterface {}

impl PlaybackInterface for GPUIPlaybackInterface {
    fn new(commands_tx: Sender<PlaybackCommand>, events_rx: Receiver<PlaybackEvent>) -> Self {
        Self {
            commands_tx,
            events_rx: Some(events_rx),
        }
    }
}

impl GPUIPlaybackInterface {
    pub fn play(&self) {
        self.commands_tx
            .send(PlaybackCommand::Play)
            .expect("could not send tx");
    }

    pub fn pause(&self) {
        self.commands_tx
            .send(PlaybackCommand::Pause)
            .expect("could not send tx");
    }

    pub fn open(&self, path: &str) {
        self.commands_tx
            .send(PlaybackCommand::Open(path.to_string()))
            .expect("could not send tx");
    }

    pub fn queue(&self, path: &str) {
        self.commands_tx
            .send(PlaybackCommand::Queue(path.to_string()))
            .expect("could not send tx");
    }

    pub fn queue_list(&self, paths: Vec<String>) {
        self.commands_tx
            .send(PlaybackCommand::QueueList(paths))
            .expect("could not send tx");
    }

    pub fn next(&self) {
        self.commands_tx
            .send(PlaybackCommand::Next)
            .expect("could not send tx");
    }

    pub fn previous(&self) {
        self.commands_tx
            .send(PlaybackCommand::Previous)
            .expect("could not send tx");
    }

    pub fn clear_queue(&self) {
        self.commands_tx
            .send(PlaybackCommand::ClearQueue)
            .expect("could not send tx");
    }

    pub fn jump(&self, index: usize) {
        self.commands_tx
            .send(PlaybackCommand::Jump(index))
            .expect("could not send tx");
    }

    pub fn seek(&self, position: f64) {
        self.commands_tx
            .send(PlaybackCommand::Seek(position))
            .expect("could not send tx");
    }

    pub fn set_volume(&self, volume: u8) {
        self.commands_tx
            .send(PlaybackCommand::SetVolume(volume))
            .expect("could not send tx");
    }

    /// Starts the broadcast loop that will read events from the playback thread and update data
    /// models accordingly. This function should be called once, and will panic if called more than
    /// once.
    pub fn start_broadcast(&mut self, cx: &mut AppContext) {
        let mut events_rx = None;
        std::mem::swap(&mut self.events_rx, &mut events_rx);

        let metadata_model: Model<Metadata> = cx.global::<Models>().metadata.clone();
        let albumart_model: Model<Option<RgbaImage>> = cx.global::<Models>().albumart.clone();

        if let Some(events_rx) = events_rx {
            cx.spawn(|mut cx| async move {
                loop {
                    while let Ok(event) = events_rx.try_recv() {
                        match event {
                            PlaybackEvent::MetadataUpdate(v) => {
                                metadata_model
                                    .update(&mut cx, |m, cx| {
                                        *m = *v;
                                        cx.notify()
                                    })
                                    .expect("failed to update metadata");
                            }
                            PlaybackEvent::AlbumArtUpdate(v) => {
                                albumart_model
                                    .update(&mut cx, |m, cx| {
                                        *m = Some(v);
                                        cx.notify()
                                    })
                                    .expect("failed to update albumart");
                            }
                            _ => (),
                        }
                    }

                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                }
            })
            .detach();
        } else {
            panic!("broadcast thread already started");
        }
    }
}

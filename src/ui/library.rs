use album_view::AlbumView;
use artist_detail_view::ArtistDetailView;
use artist_view::ArtistView;
use cntp_i18n::tr;
use gpui::{prelude::FluentBuilder, *};
use navigation::NavigationView;
use release_view::ReleaseView;
use tracing::debug;
use track_view::TrackView;

#[derive(Clone, Default)]
struct ScrollStateStorage {
    album_view_scroll: Option<f32>,
    track_view_scroll: Option<f32>,
    artist_view_scroll: Option<f32>,
}

use crate::ui::{
    command_palette::{Command, CommandManager},
    components::table::table_data::TABLE_MAX_WIDTH,
    library::{
        playlist_view::{Import, PlaylistView},
        sidebar::Sidebar,
        update_playlist::UpdatePlaylist,
    },
};

use super::models::Models;

pub mod add_to_playlist;
pub mod missing_folder_dialog;
mod album_view;
mod artist_detail_view;
mod artist_view;
mod navigation;
mod playlist_view;
mod release_view;
mod sidebar;
mod track_listing;
mod track_view;
mod update_playlist;

pub fn bind_actions(cx: &mut App) {
    playlist_view::bind_actions(cx);
}

/// The navigation history + a cursor noting what the current message is.
#[derive(Debug)]
pub struct NavigationHistory {
    history: Vec<ViewSwitchMessage>,
    cursor: usize,
}

impl NavigationHistory {
    pub fn new() -> Self {
        Self {
            history: vec![ViewSwitchMessage::Albums],
            cursor: 0,
        }
    }

    pub fn current(&self) -> ViewSwitchMessage {
        self.history[self.cursor]
    }

    pub fn can_go_back(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.cursor < self.history.len() - 1
    }

    pub fn go_back(&mut self) -> Option<ViewSwitchMessage> {
        if self.can_go_back() {
            self.cursor -= 1;
            Some(self.current())
        } else {
            None
        }
    }

    pub fn go_forward(&mut self) -> Option<ViewSwitchMessage> {
        if self.can_go_forward() {
            self.cursor += 1;
            Some(self.current())
        } else {
            None
        }
    }

    /// Navigates to a new view. All history entries after the cursor are discarded, then the new
    /// view is appended and the cursor advances to it. History is capped at 100 entries.
    pub fn navigate(&mut self, message: ViewSwitchMessage) {
        // Drop any forward history.
        self.history.truncate(self.cursor + 1);

        // Cap total history at 100 entries by evicting the oldest.
        if self.history.len() >= 100 {
            self.history.remove(0);
            self.cursor = self.cursor.saturating_sub(1);
        }

        self.history.push(message);
        self.cursor = self.history.len() - 1;
    }

    /// Removes history entries that do not satisfy `f`, adjusting the cursor so that it continues
    /// to point at the same entry if it survives, or backs up to the nearest preceding survivor
    /// otherwise. History is guaranteed to never become empty (falls back to `Albums`).
    ///
    /// Used to remove entries that are no longer valid.
    pub fn retain<F>(&mut self, f: F)
    where
        F: Fn(&ViewSwitchMessage) -> bool,
    {
        // Count how many entries at or before the cursor will be removed.
        let removed_before_or_at_cursor = self.history[..=self.cursor]
            .iter()
            .filter(|v| !f(v))
            .count();

        self.history.retain(f);

        if self.history.is_empty() {
            self.history.push(ViewSwitchMessage::Albums);
            self.cursor = 0;
        } else {
            self.cursor = self
                .cursor
                .saturating_sub(removed_before_or_at_cursor)
                .min(self.history.len() - 1);
        }
    }
}

impl Default for NavigationHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl EventEmitter<ViewSwitchMessage> for NavigationHistory {}

#[derive(Clone)]
enum LibraryView {
    Album(Entity<AlbumView>),
    Tracks(Entity<TrackView>),
    Release(Entity<ReleaseView>),
    Playlist(Entity<PlaylistView>),
    Artists(Entity<ArtistView>),
    ArtistDetail(Entity<ArtistDetailView>),
}

pub struct Library {
    view: LibraryView,
    navigation_view: Entity<NavigationView>,
    sidebar: Entity<Sidebar>,
    show_update_playlist: Entity<bool>,
    update_playlist: Entity<UpdatePlaylist>,
    focus_handle: FocusHandle,
    scroll_state: ScrollStateStorage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewSwitchMessage {
    Albums,
    Tracks,
    Artists,
    Release(i64),
    Artist(i64),
    Playlist(i64),
    Back,
    Forward,
    Refresh,
}

fn make_view(
    message: &ViewSwitchMessage,
    cx: &mut App,
    model: Entity<NavigationHistory>,
    scroll_state: &ScrollStateStorage,
) -> LibraryView {
    match message {
        ViewSwitchMessage::Albums => LibraryView::Album(AlbumView::new(
            cx,
            model.clone(),
            scroll_state.album_view_scroll,
        )),
        ViewSwitchMessage::Tracks => LibraryView::Tracks(TrackView::new(
            cx,
            model.clone(),
            scroll_state.track_view_scroll,
        )),
        ViewSwitchMessage::Artists => LibraryView::Artists(ArtistView::new(
            cx,
            model.clone(),
            scroll_state.artist_view_scroll,
        )),
        ViewSwitchMessage::Release(id) => LibraryView::Release(ReleaseView::new(cx, *id)),
        ViewSwitchMessage::Artist(id) => {
            LibraryView::ArtistDetail(ArtistDetailView::new(cx, *id, model.clone()))
        }
        ViewSwitchMessage::Playlist(id) => LibraryView::Playlist(PlaylistView::new(cx, *id)),
        ViewSwitchMessage::Back => panic!("improper use of make_view (cannot make Back)"),
        ViewSwitchMessage::Forward => panic!("improper use of make_view (cannot make Forward)"),
        ViewSwitchMessage::Refresh => panic!("improper use of make_view (cannot make Refresh)"),
    }
}

impl Library {
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let switcher_model = cx.global::<Models>().switcher_model.clone();
            let scroll_state = ScrollStateStorage::default();
            let view = LibraryView::Album(AlbumView::new(
                cx,
                switcher_model.clone(),
                scroll_state.album_view_scroll,
            ));

            cx.subscribe(
                &switcher_model,
                move |this: &mut Library, m, message, cx| {
                    // Save current scroll position before switching away.
                    if let LibraryView::Album(album_view) = &this.view {
                        let scroll_pos = album_view.read(cx).get_scroll_offset(cx);
                        this.scroll_state.album_view_scroll = Some(scroll_pos);
                    } else if let LibraryView::Tracks(track_view) = &this.view {
                        let scroll_pos = track_view.read(cx).get_scroll_offset(cx);
                        this.scroll_state.track_view_scroll = Some(scroll_pos);
                    } else if let LibraryView::Artists(artist_view) = &this.view {
                        let scroll_pos = artist_view.read(cx).get_scroll_offset(cx);
                        this.scroll_state.artist_view_scroll = Some(scroll_pos);
                    }

                    this.view = match message {
                        ViewSwitchMessage::Back => {
                            let destination =
                                m.update(cx, |history: &mut NavigationHistory, cx| {
                                    let result = history.go_back();
                                    cx.notify();
                                    result
                                });

                            if let Some(dest) = destination {
                                debug!("back → {:?}", dest);
                                make_view(&dest, cx, m, &this.scroll_state)
                            } else {
                                this.view.clone()
                            }
                        }

                        ViewSwitchMessage::Forward => {
                            let destination =
                                m.update(cx, |history: &mut NavigationHistory, cx| {
                                    let result = history.go_forward();
                                    cx.notify();
                                    result
                                });

                            if let Some(dest) = destination {
                                debug!("forward → {:?}", dest);
                                make_view(&dest, cx, m, &this.scroll_state)
                            } else {
                                this.view.clone()
                            }
                        }

                        ViewSwitchMessage::Refresh => {
                            let current = m.read(cx).current();
                            make_view(&current, cx, m, &this.scroll_state)
                        }

                        _ => {
                            m.update(cx, |history, cx| {
                                history.navigate(*message);
                                cx.notify();
                            });

                            make_view(message, cx, m, &this.scroll_state)
                        }
                    };

                    cx.notify();
                },
            )
            .detach();

            let focus_handle = cx.focus_handle();

            cx.register_command(
                ("playlist::import", 0),
                Command::new(
                    Some(tr!("ACTION_GROUP_PLAYLIST", "Playlist")),
                    tr!("ACTION_IMPORT_PLAYLIST", "Import M3U Playlist"),
                    Import,
                    Some(focus_handle.clone()),
                ),
            );

            cx.on_release(move |_, cx| {
                cx.unregister_command(("playlist::import", 0));
            })
            .detach();

            let show_update_playlist = cx.new(|_| false);

            let settings = cx.global::<crate::settings::SettingsGlobal>().model.clone();
            cx.observe(&settings, |_, _, cx| cx.notify()).detach();

            Library {
                navigation_view: NavigationView::new(cx, switcher_model.clone()),
                sidebar: Sidebar::new(cx, switcher_model.clone()),
                view,
                update_playlist: UpdatePlaylist::new(cx, show_update_playlist.clone()),
                show_update_playlist,
                focus_handle,
                scroll_state,
            }
        })
    }
}

impl Render for Library {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let show_update_playlist = self.show_update_playlist.clone();
        let settings = cx
            .global::<crate::settings::SettingsGlobal>()
            .model
            .read(cx);
        let full_width = settings.interface.full_width_library;

        div()
            .id("library")
            .track_focus(&self.focus_handle)
            .on_action(move |_: &Import, _, cx| {
                show_update_playlist.update(cx, |v, cx| {
                    *v = true;
                    cx.notify();
                })
            })
            .on_mouse_down(
                MouseButton::Navigate(gpui::NavigationDirection::Back),
                |_, _, cx| {
                    let switcher = cx.global::<Models>().switcher_model.clone();
                    switcher.update(cx, |_, cx| {
                        cx.emit(ViewSwitchMessage::Back);
                    });
                },
            )
            .on_mouse_down(
                MouseButton::Navigate(gpui::NavigationDirection::Forward),
                |_, _, cx| {
                    let switcher = cx.global::<Models>().switcher_model.clone();
                    switcher.update(cx, |_, cx| {
                        cx.emit(ViewSwitchMessage::Forward);
                    });
                },
            )
            .w_full()
            .h_full()
            .flex()
            .flex_shrink()
            .max_w_full()
            .max_h_full()
            .overflow_hidden()
            .child(
                div()
                    .mr_auto()
                    .flex()
                    .flex_shrink_0()
                    .child(self.sidebar.clone()),
            )
            .child(
                div()
                    .w_full()
                    .when(!full_width, |this: Div| this.max_w(px(TABLE_MAX_WIDTH)))
                    .h_full()
                    .flex()
                    .flex_col()
                    .flex_shrink()
                    .mr_auto()
                    .overflow_hidden()
                    .child(self.navigation_view.clone())
                    .child(match &self.view {
                        LibraryView::Album(album_view) => album_view.clone().into_any_element(),
                        LibraryView::Tracks(track_view) => track_view.clone().into_any_element(),
                        LibraryView::Release(release_view) => {
                            release_view.clone().into_any_element()
                        }
                        LibraryView::Playlist(playlist_view) => {
                            playlist_view.clone().into_any_element()
                        }
                        LibraryView::Artists(artist_view) => artist_view.clone().into_any_element(),
                        LibraryView::ArtistDetail(artist_detail_view) => {
                            artist_detail_view.clone().into_any_element()
                        }
                    }),
            )
            .child(self.update_playlist.clone())
    }
}

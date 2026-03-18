mod lrc;

use lrc::{LrcLine, parse_lrc};

use crate::{
    library::db::LibraryAccess,
    ui::{
        components::{
            icons::{MICROPHONE, icon},
            scrollbar::{RightPad, ScrollableHandle, floating_scrollbar},
        },
        models::{CurrentTrack, PlaybackInfo},
        theme::Theme,
    },
};
use cntp_i18n::tr;
use gpui::*;

pub struct Lyrics {
    content: Option<String>,
    parsed: Option<Vec<LrcLine>>,
    last_active_line: Option<usize>,
    scroll_handle: ScrollHandle,
}

impl Lyrics {
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let playback_info = cx.global::<PlaybackInfo>().clone();
            let current_track = playback_info.current_track.clone();
            let position = playback_info.position.clone();

            let initial_track = current_track.read(cx).clone();
            let (content, parsed) = Self::load_lyrics(initial_track.as_ref(), cx);

            cx.observe(&current_track, |this: &mut Lyrics, ct, cx| {
                let track = ct.read(cx).clone();
                let (content, parsed) = Self::load_lyrics(track.as_ref(), cx);
                this.content = content;
                this.parsed = parsed;
                this.last_active_line = None;
                this.scroll_handle.set_offset(gpui::Point {
                    x: px(0.0),
                    y: px(0.0),
                });
                cx.notify();
            })
            .detach();

            cx.observe(&position, |this: &mut Lyrics, pos, cx| {
                if let Some(parsed) = &this.parsed {
                    let pos_ms = *pos.read(cx) * 1_000;
                    let idx = parsed.partition_point(|l| l.time_ms <= pos_ms);
                    let new_line = if idx == 0 { None } else { Some(idx - 1) };
                    if new_line != this.last_active_line {
                        this.last_active_line = new_line;

                        if let Some(idx) = new_line {
                            if let Some(item_bounds) = this.scroll_handle.bounds_for_item(idx) {
                                let viewport = this.scroll_handle.bounds();
                                let new_offset_y = viewport.origin.y - item_bounds.origin.y
                                    + viewport.size.height / 2.0
                                    - item_bounds.size.height / 2.0;
                                this.scroll_handle.set_offset(gpui::Point {
                                    x: px(0.0),
                                    y: new_offset_y.min(px(0.0)),
                                });
                            }
                        }

                        cx.notify();
                    }
                }
            })
            .detach();

            Self {
                content,
                parsed,
                last_active_line: None,
                scroll_handle: ScrollHandle::new(),
            }
        })
    }

    fn load_lyrics(
        track: Option<&CurrentTrack>,
        cx: &App,
    ) -> (Option<String>, Option<Vec<LrcLine>>) {
        let content = track
            .and_then(|t| cx.get_track_by_path(t.get_path()).ok().flatten())
            .and_then(|t| cx.lyrics_for_track(t.id).ok().flatten());
        let parsed = content.as_ref().and_then(|c| parse_lrc(c));
        (content, parsed)
    }
}

impl Render for Lyrics {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();

        let muted = theme.text_secondary;
        let normal = theme.text;

        let inner: AnyElement = if self.content.is_none() {
            div()
                .h_full()
                .w_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .items_center()
                        .text_color(muted)
                        .child(icon(MICROPHONE).size(px(16.0)))
                        .child(tr!("NO_LYRICS", "No lyrics")),
                )
                .into_any_element()
        // LRC
        } else if let Some(parsed) = &self.parsed {
            let active_line = self.last_active_line;
            let scroll_handle = self.scroll_handle.clone();

            let items: Vec<AnyElement> = parsed
                .iter()
                .enumerate()
                .map(|(idx, line)| {
                    if line.text.is_empty() {
                        div().h(px(32.0)).w_full().into_any_element()
                    } else {
                        let is_active = Some(idx) == active_line;
                        div()
                            .px(px(20.0))
                            .py(px(11.0))
                            .text_size(px(22.0))
                            .line_height(rems(1.5))
                            .font_weight(if is_active {
                                FontWeight::EXTRA_BOLD
                            } else {
                                FontWeight::BOLD
                            })
                            .text_color(if is_active { normal } else { muted })
                            .child(SharedString::from(line.text.clone()))
                            .into_any_element()
                    }
                })
                .collect();

            div()
                .h_full()
                .w_full()
                .relative()
                .child(
                    div()
                        .id("lyrics-scroll")
                        .h_full()
                        .w_full()
                        .overflow_y_scroll()
                        .track_scroll(&scroll_handle)
                        .children(items),
                )
                .child(floating_scrollbar(
                    "lyrics-scrollbar",
                    ScrollableHandle::Regular(scroll_handle),
                    RightPad::Pad,
                ))
                .into_any_element()
        } else {
            let text = self.content.clone().unwrap();
            div()
                .id("lyrics-plain-text")
                .h_full()
                .w_full()
                .overflow_y_scroll()
                .px(px(16.0))
                .py(px(12.0))
                .text_size(px(20.0))
                .line_height(rems(1.6))
                .font_weight(FontWeight::BOLD)
                .text_color(normal)
                .child(SharedString::from(text))
                .into_any_element()
        };

        div().h_full().w_full().flex().flex_col().child(inner)
    }
}

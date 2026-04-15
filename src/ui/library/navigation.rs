use gpui::{prelude::FluentBuilder, *};
use tracing::debug;

use crate::{
    library::db::{AlbumMethod, LibraryAccess},
    ui::{
        components::{
            icons::{ARROW_LEFT, ARROW_RIGHT},
            nav_button::nav_button,
            table::table_data::TABLE_MAX_WIDTH,
        },
        theme::Theme,
    },
};

use super::{NavigationHistory, ViewSwitchMessage};

type MakeElement = dyn Fn(&mut Window, &mut Context<NavigationView>) -> AnyElement + 'static;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NavigationDisplayMode {
    Visible,
    Spacer,
}

impl NavigationDisplayMode {
    pub(super) fn shows_buttons(self) -> bool {
        matches!(self, Self::Visible)
    }
}

pub(super) struct NavigationView {
    view_switcher_model: Entity<NavigationHistory>,
    current_message: ViewSwitchMessage,
    description: Option<SharedString>,
    display_mode: NavigationDisplayMode,
    left: Option<Box<MakeElement>>,
    right: Option<Box<MakeElement>>,
}

impl NavigationView {
    pub(super) fn new(
        cx: &mut App,
        view_switcher_model: Entity<NavigationHistory>,
        display_mode: NavigationDisplayMode,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let current_message = view_switcher_model.read(cx).current();

            cx.observe(&view_switcher_model, |this: &mut NavigationView, m, cx| {
                debug!("{:#?}", m.read(cx));

                this.current_message = m.read(cx).current();

                this.description = match this.current_message {
                    ViewSwitchMessage::Release(id, _) => cx
                        .get_album_by_id(id, AlbumMethod::Metadata)
                        .ok()
                        .map(|v| SharedString::from(v.title.clone())),
                    ViewSwitchMessage::Artist(id) => cx
                        .get_artist_by_id(id)
                        .ok()
                        .and_then(|v| v.name.as_ref().map(|n| n.0.clone())),
                    _ => None,
                }
            })
            .detach();

            Self {
                view_switcher_model,
                current_message,
                description: None,
                display_mode,
                left: None,
                right: None,
            }
        })
    }

    pub(super) fn set_left(
        &mut self,
        f: impl Fn(&mut Window, &mut Context<Self>) -> AnyElement + 'static,
        cx: &mut Context<Self>,
    ) {
        self.left = Some(Box::new(f));
        cx.notify();
    }

    pub(super) fn set_right(
        &mut self,
        f: impl Fn(&mut Window, &mut Context<Self>) -> AnyElement + 'static,
        cx: &mut Context<Self>,
    ) {
        self.right = Some(Box::new(f));
        cx.notify();
    }

    pub(super) fn set_display_mode(
        &mut self,
        display_mode: NavigationDisplayMode,
        cx: &mut Context<Self>,
    ) {
        if self.display_mode != display_mode {
            self.display_mode = display_mode;
            cx.notify();
        }
    }
}

impl Render for NavigationView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shows_buttons = self.display_mode.shows_buttons();

        let (can_go_back, can_go_forward, full_width) = if shows_buttons {
            let can_go_back = self.view_switcher_model.read(cx).can_go_back();
            let can_go_forward = self.view_switcher_model.read(cx).can_go_forward();
            let settings = cx
                .global::<crate::settings::SettingsGlobal>()
                .model
                .read(cx);
            let full_width = settings.interface.effective_full_width();
            (can_go_back, can_go_forward, full_width)
        } else {
            (false, false, false)
        };

        let left_element = self.left.as_ref().map(|f| f(window, cx));
        let right_element = self.right.as_ref().map(|f| f(window, cx));
        let has_slots = left_element.is_some() || right_element.is_some();

        let theme = cx.global::<Theme>();

        div()
            .flex()
            .border_b_1()
            .border_color(theme.border_color)
            .w_full()
            .when(shows_buttons, |d| {
                d.child(
                    div()
                        .flex()
                        .gap(px(4.0))
                        .when(!full_width, |this: Div| this.max_w(px(TABLE_MAX_WIDTH)))
                        .px(px(10.0))
                        .py(px(10.0))
                        .child(
                            nav_button("back", ARROW_LEFT)
                                .disabled(!can_go_back)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.view_switcher_model.update(cx, |_, cx| {
                                        cx.emit(ViewSwitchMessage::Back);
                                    })
                                })),
                        )
                        .child(
                            nav_button("forward", ARROW_RIGHT)
                                .disabled(!can_go_forward)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.view_switcher_model.update(cx, |_, cx| {
                                        cx.emit(ViewSwitchMessage::Forward);
                                    })
                                })),
                        ),
                )
            })
            .when(has_slots, |d| {
                d.child(
                    div()
                        .border_l_1()
                        .border_color(theme.border_color)
                        .min_h(px(48.0))
                        .w_full()
                        .py(px(10.0))
                        .pl(px(18.0))
                        .pr(px(12.0))
                        .flex()
                        .justify_between()
                        .items_center()
                        .when_some(left_element, |d, el| d.child(el))
                        .when_some(right_element, |d, el| d.child(el)),
                )
            })
    }
}

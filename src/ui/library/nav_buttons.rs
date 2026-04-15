use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, StatefulInteractiveElement,
    Styled, Window, div, px,
};

use crate::ui::{
    components::{
        icons::{ARROW_LEFT, ARROW_RIGHT},
        nav_button::nav_button,
    },
    models::Models,
};

use super::ViewSwitchMessage;

#[derive(IntoElement)]
pub struct NavButtons {}

impl RenderOnce for NavButtons {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let vsm = cx.global::<Models>().switcher_model.clone();
        let can_go_back = vsm.read(cx).can_go_back();
        let can_go_forward = vsm.read(cx).can_go_forward();

        div()
            .flex()
            .occlude()
            .mt(px(1.0))
            .mr(px(6.0))
            .gap(px(4.0))
            .child(
                nav_button("back", ARROW_LEFT)
                    .disabled(!can_go_back)
                    .on_click({
                        let vsm = vsm.clone();
                        move |_, _, cx| {
                            vsm.update(cx, |_, cx| {
                                cx.emit(ViewSwitchMessage::Back);
                            })
                        }
                    }),
            )
            .child(
                nav_button("forward", ARROW_RIGHT)
                    .disabled(!can_go_forward)
                    .on_click({
                        let vsm = vsm.clone();
                        move |_, _, cx| {
                            vsm.update(cx, |_, cx| {
                                cx.emit(ViewSwitchMessage::Forward);
                            })
                        }
                    }),
            )
    }
}

pub fn nav_buttons() -> impl IntoElement {
    NavButtons {}
}

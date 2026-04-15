use gpui::{prelude::FluentBuilder, *};

use crate::ui::{
    components::{
        icons::{GRID, GRID_INACTIVE, LIST, LIST_INACTIVE},
        nav_button::nav_button,
        table::table_data::TableData,
        table::{Table, TableViewMode},
        tooltip::build_tooltip,
    },
    theme::Theme,
};

use cntp_i18n::tr;

use super::{NavigationDisplayMode, NavigationHistory, navigation::NavigationView};

pub struct TableViewHeader<T, C>
where
    T: TableData<C> + 'static,
    C: crate::ui::components::table::table_data::Column + 'static,
{
    navigation_view: Entity<NavigationView>,
    _table: std::marker::PhantomData<(T, C)>,
}

impl<T, C> TableViewHeader<T, C>
where
    T: TableData<C> + 'static,
    C: crate::ui::components::table::table_data::Column + 'static,
{
    pub fn new(
        cx: &mut App,
        view_switch_model: Entity<NavigationHistory>,
        navigation_mode: NavigationDisplayMode,
        table: Entity<Table<T, C>>,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let navigation_view = NavigationView::new(cx, view_switch_model, navigation_mode);

            let table_for_left = table.clone();
            navigation_view.update(cx, |nv, cx| {
                nv.set_left(
                    move |_, _| {
                        div()
                            .line_height(px(26.0))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(18.0))
                            .child(Table::<T, C>::get_table_name())
                            .into_any_element()
                    },
                    cx,
                );

                if T::supports_grid_view() {
                    let table_ref = table_for_left.clone();
                    nv.set_right(
                        move |_, cx| {
                            let view_mode = table_ref.read(cx).get_view_mode(cx);
                            let is_grid = view_mode == TableViewMode::Grid;
                            let theme = cx.global::<Theme>();
                            let nav_button_pressed = theme.nav_button_pressed;
                            let nav_button_pressed_border = theme.nav_button_pressed_border;
                            let table_for_list = table_ref.clone();
                            let table_for_grid = table_ref.clone();

                            div()
                                .flex()
                                .gap_1()
                                .child(
                                    nav_button(
                                        "list_toggle",
                                        if !is_grid { LIST } else { LIST_INACTIVE },
                                    )
                                    .on_click(move |_, _, cx| {
                                        table_for_list.update(cx, |t, cx| {
                                            t.set_view_mode(TableViewMode::List, cx);
                                        });
                                    })
                                    .when(!is_grid, |this| {
                                        this.bg(nav_button_pressed)
                                            .border_color(nav_button_pressed_border)
                                    })
                                    .tooltip(build_tooltip(tr!("LIST_VIEW", "List View"))),
                                )
                                .child(
                                    nav_button(
                                        "grid_toggle",
                                        if is_grid { GRID } else { GRID_INACTIVE },
                                    )
                                    .on_click(move |_, _, cx| {
                                        table_for_grid.update(cx, |t, cx| {
                                            t.set_view_mode(TableViewMode::Grid, cx);
                                        });
                                    })
                                    .when(is_grid, |this| {
                                        this.bg(nav_button_pressed)
                                            .border_color(nav_button_pressed_border)
                                    })
                                    .tooltip(build_tooltip(tr!("GRID_VIEW", "Grid View"))),
                                )
                                .into_any_element()
                        },
                        cx,
                    );
                }
            });

            Self {
                navigation_view,
                _table: std::marker::PhantomData,
            }
        })
    }

    pub fn set_navigation_display_mode(
        &mut self,
        display_mode: NavigationDisplayMode,
        cx: &mut Context<Self>,
    ) {
        self.navigation_view.update(cx, |nv, cx| {
            nv.set_display_mode(display_mode, cx);
        });
    }
}

impl<T, C> Render for TableViewHeader<T, C>
where
    T: TableData<C> + 'static,
    C: crate::ui::components::table::table_data::Column + 'static,
{
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.navigation_view.clone()
    }
}

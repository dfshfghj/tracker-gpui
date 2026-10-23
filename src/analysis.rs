//! Docked and floating analysis use the same views and document state.
use std::collections::BTreeMap;

use gpui_component::resizable::{resizable_panel, v_resizable, ResizableState};
use gpui_component::scroll::ScrollableElement;
use gpui_kit::prelude::FluentBuilder;

use crate::i18n::tr;
use crate::*;

// Notify the window's model view without forcing all retained child views to
// repaint. The calling window is already borrowed, so notify it directly first.
pub(crate) fn notify_workspace_windows(cx: &mut Context<Probe>) {
    cx.notify();
    for handle in cx.windows() {
        notify_window(handle, cx);
    }
}
fn notify_window(handle: AnyWindowHandle, cx: &mut App) {
    let _ = handle.update(cx, |root, _, cx| {
        let view = root
            .clone()
            .downcast::<gpui_kit::base::Root>()
            .map(|root| root.read(cx).view().clone())
            .unwrap_or(root);
        cx.notify(view.entity_id());
    });
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Pane {
    Plot(usize),
    Table,
    Variables,
}

impl Pane {
    fn title(self) -> String {
        match self {
            Self::Plot(index) => rust_i18n::t!("analysis.plot_number", n = index + 1).to_string(),
            Self::Table => tr("inspector.data_table"),
            Self::Variables => tr("variables.title"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Section {
    Plots,
    Table,
}

#[derive(Default)]
pub(crate) struct PaneWindows {
    pub windows: BTreeMap<Pane, AnyWindowHandle>,
}

pub(crate) fn split_ratio(value: Option<f32>) -> f32 {
    value
        .filter(|v| v.is_finite())
        .unwrap_or(0.6)
        .clamp(0.15, 0.85)
}

impl Probe {
    pub(crate) fn analysis_instance(
        state: Rc<RefCell<State>>,
        role: Option<Pane>,
        windows: Rc<RefCell<PaneWindows>>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            state,
            focus: cx.focus_handle(),
            table_focus: cx.focus_handle(),
            variable_editor: None,
            analysis_role: role,
            analysis_windows: windows,
            analysis_split: None,
            analysis_maximized: None,
            table_scroll: UniformListScrollHandle::new(),
            table_x_scroll: ScrollHandle::new(),
            plot_scroll: ScrollHandle::new(),
            plot_cache: crate::plot_view::PlotCache::default(),
            table_columns_open: false,
            plot_var_menu: None,
            plot_var_pos: (0., 0.),
            focused_once: false,
            dialog_only: false,
            main: None,
            close_hook: false,
            pending_tracker_window: false,
            tracker_window: None,
            pending_settings_window: false,
            settings_window: None,
            panel_layout: None,
            main_hwnd: None,
            owner_hwnd: None,
            owner_applied: false,
        }
    }

    pub(crate) fn observe_analysis_owner(cx: &Context<Self>) {
        cx.on_release(|this, cx| {
            let windows = std::mem::take(&mut this.analysis_windows.borrow_mut().windows);
            for (_, handle) in windows {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            }
        })
        .detach();
    }

    pub(crate) fn is_detached(&self, pane: Pane) -> bool {
        self.analysis_windows.borrow().windows.contains_key(&pane)
    }

    pub(crate) fn open_analysis_window(&mut self, pane: Pane, cx: &mut Context<Self>) {
        self.plot_var_menu = None;
        let existing = self.analysis_windows.borrow().windows.get(&pane).copied();
        if let Some(handle) = existing {
            if handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
            {
                return;
            }
            self.analysis_windows.borrow_mut().windows.remove(&pane);
        }
        let state = self.state.clone();
        let windows = self.analysis_windows.clone();
        let table_scroll = self.table_scroll.clone();
        let table_x_scroll = self.table_x_scroll.clone();
        let owner = cx.entity().downgrade();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(180.), px(90.)),
                size: match pane {
                    Pane::Table => size(px(860.), px(640.)),
                    _ => size(px(800.), px(600.)),
                },
            })),
            titlebar: Some(TitlebarOptions {
                title: Some(pane.title().into()),
                appears_transparent: false,
                traffic_light_position: None,
            }),
            kind: WindowKind::Normal,
            focus: true,
            show: true,
            is_resizable: true,
            is_movable: true,
            ..Default::default()
        };
        match gpui_kit::open_window(options, cx, move |window, cx| {
            let handle = window.window_handle();
            let id = handle.window_id();
            // GPUI draws synchronously inside open_window, before returning its handle.
            windows.borrow_mut().windows.insert(pane, handle);
            cx.new(|cx| {
                let registry = windows.clone();
                let owner = owner.clone();
                cx.on_release(move |_, cx| {
                    // A delayed release from an older window cannot remove its replacement.
                    let mut registry = registry.borrow_mut();
                    if registry
                        .windows
                        .get(&pane)
                        .is_some_and(|h| h.window_id() == id)
                    {
                        registry.windows.remove(&pane);
                    }
                    drop(registry);
                    let _ = owner.update(cx, |_, cx| cx.notify());
                    cx.refresh_windows();
                })
                .detach();
                let mut view =
                    Self::analysis_instance(state.clone(), Some(pane), windows.clone(), cx);
                if pane == Pane::Table {
                    view.table_scroll = table_scroll.clone();
                    view.table_x_scroll = table_x_scroll.clone();
                }
                if pane == Pane::Variables {
                    view.variable_editor = Some(cx.new(|cx| {
                        crate::variable_editor::VariableEditor::new(state.clone(), window, cx)
                    }));
                }
                view
            })
        }) {
            Ok((handle, _)) => {
                self.analysis_windows
                    .borrow_mut()
                    .windows
                    .insert(pane, handle);
            }
            Err(error) => {
                self.state.borrow_mut().status =
                    Some(rust_i18n::t!("analysis.window_failed", detail = error).to_string());
            }
        }
        notify_workspace_windows(cx);
    }

    pub(crate) fn dock_analysis_window(&mut self, pane: Pane, cx: &mut Context<Self>) {
        let handle = self.analysis_windows.borrow_mut().windows.remove(&pane);
        if let Some(handle) = handle {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
        notify_workspace_windows(cx);
    }

    pub(crate) fn refresh_analysis_windows(&self, cx: &mut Context<Self>) {
        let handles: Vec<_> = self
            .analysis_windows
            .borrow()
            .windows
            .values()
            .copied()
            .collect();
        for handle in handles {
            notify_window(handle, cx);
        }
    }

    pub(crate) fn detached_placeholder(&self, pane: Pane, cx: &mut Context<Self>) -> AnyElement {
        div()
            .w_full()
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
            .py_2()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_xs()
                    .text_color(c(0x7d8ca6))
                    .child(rust_i18n::t!("analysis.detached", detail = pane.title()).to_string()),
            )
            .child(icon_button(
                "focus-analysis-window",
                ICON_DETACH,
                tr("analysis.show_window"),
                24.,
                15.,
                false,
                true,
                cx,
                move |this, cx| this.open_analysis_window(pane, cx),
            ))
            .child(icon_button(
                "return-analysis-window",
                ICON_DOCK,
                tr("analysis.dock"),
                24.,
                15.,
                false,
                true,
                cx,
                move |this, cx| this.dock_analysis_window(pane, cx),
            ))
            .into_any_element()
    }

    fn analysis_header(&self, section: Section, cx: &mut Context<Self>) -> AnyElement {
        let maximized = self.analysis_maximized == Some(section);
        let mut header = div()
            .flex_none()
            .w_full()
            .flex()
            .items_center()
            .gap_2()
            .py_1()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(0x8fa3bf))
                    .child(match section {
                        Section::Plots => tr("inspector.plot"),
                        Section::Table => tr("inspector.data_table"),
                    }),
            );
        if section == Section::Table {
            let floating = self.analysis_role == Some(Pane::Table);
            header = header.child(icon_button(
                "detach-data-table",
                if floating { ICON_DOCK } else { ICON_DETACH },
                if floating {
                    tr("analysis.dock")
                } else {
                    tr("analysis.detach")
                },
                24.,
                15.,
                false,
                true,
                cx,
                move |this, cx| {
                    if floating {
                        this.dock_analysis_window(Pane::Table, cx)
                    } else {
                        this.open_analysis_window(Pane::Table, cx)
                    }
                },
            ));
        }
        if self.analysis_role.is_none() {
            header = header.child(icon_button(
                "maximize-analysis-section",
                if maximized {
                    ICON_RESTORE
                } else {
                    ICON_MAXIMIZE
                },
                if maximized {
                    tr("analysis.restore")
                } else {
                    tr("analysis.maximize")
                },
                24.,
                15.,
                false,
                true,
                cx,
                move |this, cx| {
                    this.analysis_maximized = if this.analysis_maximized == Some(section) {
                        None
                    } else {
                        Some(section)
                    };
                    cx.notify();
                },
            ));
        }
        header.into_any_element()
    }

    pub(crate) fn render_analysis_workspace(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let variables = plot_chip(900, tr("variables.title"), false, cx, |this, cx| {
            this.open_analysis_window(Pane::Variables, cx)
        });
        let tools = div()
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_xs()
                    .text_color(c(0x7d8ca6))
                    .child(tr("analysis.current_track")),
            )
            .child(variables);
        let plot_header = self.analysis_header(Section::Plots, cx);
        let plot_body = crate::plot_view::plot_sidebar(
            self.state.clone(),
            None,
            self.analysis_windows.clone(),
            self.plot_scroll.clone(),
            &mut self.plot_cache,
            cx,
        );
        let plots = div()
            .size_full()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .child(plot_header)
            .child(plot_body)
            .into_any_element();
        let table = self.render_analysis_table(cx);
        let content = match self.analysis_maximized {
            Some(Section::Plots) => plots,
            Some(Section::Table) => table,
            None => {
                let split = self
                    .analysis_split
                    .get_or_insert_with(|| cx.new(|_| ResizableState::default()));
                let ratio = split_ratio(self.state.borrow().prefs.analysis_split_ratio);
                let state = self.state.clone();
                let group = v_resizable("analysis-split")
                    .with_state(split)
                    .child(
                        resizable_panel()
                            .size(px(600. * ratio))
                            .size_range(px(100.)..Pixels::MAX)
                            .child(plots),
                    )
                    .child(
                        resizable_panel()
                            .size(px(600. * (1. - ratio)))
                            .size_range(px(120.)..Pixels::MAX)
                            .child(table),
                    )
                    .on_resize(move |panels, _, cx| {
                        let sizes = panels.read(cx).sizes();
                        if sizes.len() == 2 && sizes[0] + sizes[1] > px(0.) {
                            let mut st = state.borrow_mut();
                            st.prefs.analysis_split_ratio = Some(split_ratio(Some(
                                f32::from(sizes[0]) / f32::from(sizes[0] + sizes[1]),
                            )));
                            st.prefs.save();
                        }
                    })
                    .into_any_element();
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_hidden()
                    .child(group)
                    .into_any_element()
            }
        };
        div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_1()
            .overflow_hidden()
            .child(tools)
            .child(content)
            .into_any_element()
    }

    pub(crate) fn render_analysis_table(&self, cx: &mut Context<Self>) -> AnyElement {
        let header = self.analysis_header(Section::Table, cx);
        if self.analysis_role.is_none() && self.is_detached(Pane::Table) {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .child(header)
                .child(self.detached_placeholder(Pane::Table, cx))
                .into_any_element();
        }
        let st = self.state.borrow();
        let has_track = st.active_track().is_some_and(|t| t.is_point_mass());
        let count = st
            .active_track()
            .filter(|t| t.is_point_mass())
            .map_or(0, |t| t.step_count());
        let mut columns = div()
            .id("analysis-table-columns")
            .flex_none()
            .max_h(px(96.))
            .overflow_y_scroll()
            .flex()
            .flex_wrap()
            .gap_2()
            .py_1();
        for (index, label) in ["t", "x", "y", "vx", "vy"].into_iter().enumerate() {
            columns = columns.child(check_item(
                600 + index,
                label,
                st.table_columns[index],
                has_track,
                cx,
                move |this, cx| {
                    let mut st = this.state.borrow_mut();
                    st.table_columns[index] = !st.table_columns[index];
                    st.sync_table_selection();
                    drop(st);
                    notify_workspace_windows(cx);
                },
            ));
        }
        if let Some(track) = st.active_track() {
            for (index, def) in track.variables.iter().enumerate() {
                let id = def.id;
                columns = columns.child(check_item(
                    1000 + index,
                    def.name.clone(),
                    def.table_visible,
                    has_track,
                    cx,
                    move |this, cx| {
                        let mut st = this.state.borrow_mut();
                        if let Some(def) = st
                            .active_track_mut()
                            .and_then(|t| t.variables.iter_mut().find(|d| d.id == id))
                        {
                            def.table_visible = !def.table_visible;
                        }
                        st.sync_table_selection();
                        st.commit_edit();
                        drop(st);
                        notify_workspace_windows(cx);
                    },
                ));
            }
        }
        let tools = div()
            .flex_none()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(plot_chip(
                610,
                tr("inspector.columns"),
                self.table_columns_open,
                cx,
                |this, _| this.table_columns_open = !this.table_columns_open,
            ))
            .child(div().flex_1());

        let data = if has_track && count > 0 {
            crate::data_table::table_view(
                &st,
                &self.table_focus,
                &self.table_scroll,
                &self.table_x_scroll,
                cx,
            )
        } else {
            div()
                .text_xs()
                .text_color(c(0x7d8ca6))
                .child(if has_track {
                    tr("inspector.no_points")
                } else {
                    tr("inspector.plot_only_point")
                })
                .into_any_element()
        };
        div()
            .size_full()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .gap_1()
            .child(header)
            .child(tools)
            .when(self.table_columns_open, |root| root.child(columns))
            .child(data)
            .into_any_element()
    }

    pub(crate) fn render_analysis_window(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !self.focused_once {
            self.focused_once = true;
            window.focus(&self.focus, cx);
        }
        let pane = self.analysis_role.unwrap();
        // Programmatic docking can remove the registry entry before the platform closes the window.
        if !self.is_detached(pane) {
            window.remove_window();
            return div().into_any_element();
        }
        let content = match pane {
            Pane::Plot(index) => crate::plot_view::plot_sidebar(
                self.state.clone(),
                Some(index),
                self.analysis_windows.clone(),
                self.plot_scroll.clone(),
                &mut self.plot_cache,
                cx,
            ),
            Pane::Table => self.render_analysis_table(cx),
            Pane::Variables => div()
                .id("variable-window-content")
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .child(self.variable_editor.as_ref().unwrap().clone())
                .into_any_element(),
        };
        let popup = crate::menus::plot_var_popup(
            self.state.clone(),
            self.plot_var_menu,
            self.plot_var_pos,
            window,
            cx,
        );
        div()
            .size_full()
            .relative()
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                if ev.keystroke.key == "escape" && this.plot_var_menu.take().is_some() {
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .flex()
            .flex_col()
            .p_3()
            .bg(c(0x0d1420))
            .text_color(c(0xdbe6f5))
            .child(content)
            .when(self.plot_var_menu.is_some(), |root| {
                root.child(
                    crate::overlay::overlay_surface()
                        .absolute()
                        .top(px(0.))
                        .left(px(0.))
                        .size_full()
                        .on_any_mouse_down(cx.listener(|this, _, _, cx| {
                            this.plot_var_menu = None;
                            cx.notify();
                        })),
                )
            })
            .child(popup)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{split_ratio, Pane, PaneWindows};
    use crate::{prefs::Preferences, Probe, State};
    use ::gpui;
    use core::prelude::v1::test;
    use gpui_kit::{AppContext, TestAppContext};
    use std::{cell::RefCell, rc::Rc};
    #[test]
    fn split_preferences_handle_missing_corrupt_and_extreme_values() {
        assert_eq!(split_ratio(None), 0.6);
        assert_eq!(split_ratio(Some(f32::NAN)), 0.6);
        assert_eq!(split_ratio(Some(f32::INFINITY)), 0.6);
        assert_eq!(split_ratio(Some(-1.)), 0.15);
        assert_eq!(split_ratio(Some(1.)), 0.85);
        assert_eq!(split_ratio(Some(0.42)), 0.42);
    }

    #[gpui::test]
    fn floating_windows_share_data_return_on_close_and_ignore_stale_releases(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_kit::init);
        let state = Rc::new(RefCell::new(
            State::with_preferences(Preferences::default()),
        ));
        // A large table must exercise the real lazy list, including its first draw.
        let mut track = crate::Track::point_mass(1, "Large track", 0);
        for frame in (0..100_000).step_by(2) {
            track.set_point(frame, frame as f32, 2.);
        }
        state.borrow_mut().tracks.push(track);
        crate::data_table::RENDERED_ROWS.with(|count| count.set(0));
        let registry = Rc::new(RefCell::new(PaneWindows::default()));
        let owner = cx.new(|cx| {
            Probe::observe_analysis_owner(cx);
            Probe::analysis_instance(state.clone(), None, registry.clone(), cx)
        });
        owner.update(cx, |view, cx| view.open_analysis_window(Pane::Table, cx));
        assert!(
            registry.borrow().windows.contains_key(&Pane::Table),
            "open failed: {:?}",
            state.borrow().status
        );
        crate::data_table::RENDERED_ROWS.with(|count| {
            assert!(count.get() > 0, "the table was not drawn");
            assert!(
                count.get() < 200,
                "rendered {} rows for a 640px window",
                count.get()
            );
        });
        let first = registry.borrow().windows[&Pane::Table];
        owner.update(cx, |view, cx| view.open_analysis_window(Pane::Table, cx));
        assert_eq!(
            registry.borrow().windows.len(),
            1,
            "opening twice duplicated the table"
        );
        assert_eq!(registry.borrow().windows[&Pane::Table], first);

        let old_view = first
            .update(cx, |root, _, cx| {
                root.downcast::<gpui_kit::base::Root>()
                    .unwrap()
                    .read(cx)
                    .view()
                    .clone()
                    .downcast::<Probe>()
                    .unwrap()
            })
            .unwrap();
        old_view.update(cx, |view, _| {
            assert!(
                Rc::ptr_eq(&view.state, &state),
                "detachment copied the document"
            );
            view.state.borrow_mut().frame = 17;
        });
        assert_eq!(state.borrow().frame, 17);
        // Retain the old view while its platform window closes: release may happen later.
        first
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
        owner.update(cx, |view, cx| view.open_analysis_window(Pane::Table, cx));
        let replacement = registry.borrow().windows[&Pane::Table];
        assert_ne!(first, replacement);
        drop(old_view);
        cx.run_until_parked();
        assert_eq!(
            registry.borrow().windows[&Pane::Table],
            replacement,
            "stale release removed the replacement window"
        );

        replacement
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
        assert!(
            !registry.borrow().windows.contains_key(&Pane::Table),
            "closing did not return the table to its dock"
        );

        owner.update(cx, |view, cx| {
            view.open_analysis_window(Pane::Plot(0), cx);
            view.open_analysis_window(Pane::Table, cx);
        });
        owner.update(cx, |view, cx| view.dock_analysis_window(Pane::Table, cx));
        cx.run_until_parked();
        assert!(!registry.borrow().windows.contains_key(&Pane::Table));
        assert!(registry.borrow().windows.contains_key(&Pane::Plot(0)));
        drop(owner);
        // Entity release listeners are drained at an App update boundary.
        cx.update(|_| {});
        cx.run_until_parked();
        assert!(
            registry.borrow().windows.is_empty(),
            "owner shutdown left detached windows alive"
        );
    }
}

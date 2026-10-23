//! Object hierarchy and selected-object properties have separate scroll/layout state.
use crate::i18n::tr;
use crate::*;
use gpui_component::{
    input::{InputEvent, InputState},
    resizable::{resizable_panel, v_resizable, ResizableState},
    scroll::ScrollableElement,
};
use gpui_kit::prelude::FluentBuilder;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Object {
    Video,
    Axes,
    Stick(usize),
    Track(u64),
}
impl Object {
    fn selected(st: &DocState) -> Self {
        match st.selection {
            Selection::None => Self::Video,
            Selection::Axes => Self::Axes,
            Selection::Stick(i) => Self::Stick(i),
            Selection::Track(id) => Self::Track(id),
        }
    }
}
impl DocState {
    pub(crate) fn object_name(&self, object: Object) -> Option<String> {
        Some(match object {
            Object::Video => self.title.clone(),
            Object::Axes => {
                if self.coords.name.is_empty() {
                    tr("object.axes")
                } else {
                    self.coords.name.clone()
                }
            }
            Object::Stick(i) => {
                let name = if i == 0 {
                    &self.calibration.name
                } else {
                    &self.calibration.extra_sticks.get(i - 1)?.name
                };
                if name.is_empty() {
                    rust_i18n::t!("object.stick", n = i + 1).to_string()
                } else {
                    name.clone()
                }
            }
            Object::Track(id) => self.tracks.iter().find(|t| t.id == id)?.name.clone(),
        })
    }
    pub(crate) fn rename_object(&mut self, object: Object, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(tr("inspector.invalid_name"));
        }
        if self.object_name(object).as_deref() == Some(name) {
            return Ok(());
        }
        match object {
            Object::Video => return Ok(()),
            Object::Axes => self.coords.name = name.into(),
            Object::Stick(0) => self.calibration.name = name.into(),
            Object::Stick(i) => {
                let Some(stick) = self.calibration.extra_sticks.get_mut(i - 1) else {
                    return Ok(());
                };
                stick.name = name.into();
            }
            Object::Track(id) => {
                let Some(track) = self.tracks.iter_mut().find(|t| t.id == id) else {
                    return Ok(());
                };
                track.name = name.into();
            }
        }
        self.commit_edit();
        Ok(())
    }
    fn select_object(&mut self, object: Object) {
        self.selection = match object {
            Object::Video => Selection::None,
            Object::Axes => Selection::Axes,
            Object::Stick(i) => {
                self.active_stick = i;
                Selection::Stick(i)
            }
            Object::Track(id) => {
                if let Some(i) = self.tracks.iter().position(|t| t.id == id) {
                    self.active = i;
                    self.sync_plot_for_active();
                }
                Selection::Track(id)
            }
        };
    }
    fn object_visible(&self, object: Object) -> bool {
        match object {
            Object::Video => true,
            Object::Axes => self.axes_visible,
            Object::Stick(i) => self.calibration.stick_visible(i),
            Object::Track(id) => self
                .tracks
                .iter()
                .find(|t| t.id == id)
                .is_some_and(|t| t.visible),
        }
    }
    fn toggle_object_visible(&mut self, object: Object) {
        match object {
            Object::Video => return,
            Object::Axes => self.axes_visible = !self.axes_visible,
            Object::Stick(i) => self
                .calibration
                .set_stick_visible(i, !self.calibration.stick_visible(i)),
            Object::Track(id) => {
                if let Some(t) = self.tracks.iter_mut().find(|t| t.id == id) {
                    t.visible = !t.visible;
                }
            }
        };
        self.commit_edit();
    }
}

pub(crate) struct Inspector {
    doc: Option<u64>,
    focus: Option<FocusHandle>,
    expanded: HashSet<Object>,
    scroll: UniformListScrollHandle,
    split: Option<Entity<ResizableState>>,
    name: Option<(u64, Object, Entity<InputState>)>,
    error: Option<String>,
}
impl Default for Inspector {
    fn default() -> Self {
        Self {
            doc: None,
            focus: None,
            expanded: [Object::Video].into(),
            scroll: UniformListScrollHandle::new(),
            split: None,
            name: None,
            error: None,
        }
    }
}
#[derive(Clone)]
struct Row {
    owner: Object,
    label: String,
    depth: usize,
}
fn tree_rows(st: &DocState, expanded: &HashSet<Object>) -> Vec<Row> {
    let mut rows = vec![Row {
        owner: Object::Video,
        label: st.title.clone(),
        depth: 0,
    }];
    if expanded.contains(&Object::Video) {
        let objects = std::iter::once(Object::Axes)
            .chain((0..st.calibration.stick_count()).map(Object::Stick))
            .chain(st.tracks.iter().map(|t| Object::Track(t.id)));
        for object in objects {
            rows.push(Row {
                owner: object,
                label: st.object_name(object).unwrap_or_default(),
                depth: 1,
            });
        }
    }
    rows
}

impl Probe {
    fn begin_object_rename(
        &mut self,
        object: Object,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<InputState>> {
        if object == Object::Video {
            return None;
        }
        if let Some((doc, previous, input)) = &self.inspector.name {
            if *previous != object {
                let text = input.read(cx).value().to_string();
                let mut st = self.state.borrow_mut();
                if st.id == *doc {
                    let _ = st.rename_object(*previous, &text);
                }
            }
        }
        let (doc, name) = {
            let st = self.state.borrow();
            (st.id, st.object_name(object)?)
        };
        if self
            .inspector
            .name
            .as_ref()
            .is_none_or(|(id, key, _)| *id != doc || *key != object)
        {
            let input = cx.new(|cx| {
                let mut input =
                    InputState::new(window, cx).placeholder(tr("inspector.rename_hint"));
                input.set_value(name.clone(), window, cx);
                input
            });
            cx.subscribe(&input, move |this, input, event: &InputEvent, cx| {
                if !this
                    .inspector
                    .name
                    .as_ref()
                    .is_some_and(|(id, key, current)| {
                        *id == doc && *key == object && current.entity_id() == input.entity_id()
                    })
                {
                    return;
                }
                if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                    let name = input.read(cx).value().to_string();
                    let result = {
                        let mut st = this.state.borrow_mut();
                        if st.id != doc {
                            return;
                        }
                        st.rename_object(object, &name)
                    };
                    this.inspector.error = result.as_ref().err().cloned();
                    if result.is_ok() || matches!(event, InputEvent::Blur) {
                        this.inspector.name = None;
                    }
                    crate::analysis::notify_workspace_windows(cx);
                }
            })
            .detach();
            self.inspector.name = Some((doc, object, input));
            self.inspector.error = None;
        }
        let input = self.inspector.name.as_ref()?.2.clone();
        let pending = input.clone();
        let owner = cx.entity().downgrade();
        if let Some(index) = tree_rows(&self.state.borrow(), &self.inspector.expanded)
            .iter()
            .position(|row| row.owner == object)
        {
            self.inspector
                .scroll
                .scroll_to_item(index, ScrollStrategy::Nearest);
        }
        window.on_next_frame(move |window, cx| {
            let active = owner
                .update(cx, |this, _| {
                    this.inspector
                        .name
                        .as_ref()
                        .is_some_and(|(_, _, input)| input.entity_id() == pending.entity_id())
                })
                .unwrap_or(false);
            if active {
                pending.update(cx, |input, cx| {
                    input.focus(window, cx);
                    input.select_all(window, cx);
                });
            }
        });
        cx.notify();
        Some(input)
    }

    pub(crate) fn render_object_inspector(
        &mut self,
        object_rows: Vec<AnyElement>,
        video_rows: Vec<AnyElement>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (doc, selected, custom) = {
            let st = self.state.borrow();
            (
                st.id,
                Object::selected(&st),
                matches!(st.selection,Selection::Track(id) if st.tracks.iter().any(|t|t.id==id&&t.is_point_mass())),
            )
        };
        if self.inspector.doc != Some(doc) {
            self.inspector.doc = Some(doc);
            self.inspector.expanded = [Object::Video].into();
            self.inspector.name = None;
            self.inspector.error = None;
            self.inspector.scroll = UniformListScrollHandle::new();
        }
        if custom && self.variable_editor.is_none() {
            self.variable_editor = Some(cx.new(|cx| {
                crate::variable_editor::VariableEditor::new(self.state.clone(), window, cx)
            }));
        }
        if self.inspector.focus.is_none() {
            self.inspector.focus = Some(cx.focus_handle());
        }
        let rows = Rc::new(tree_rows(&self.state.borrow(), &self.inspector.expanded));
        let view = cx.entity().downgrade();
        let list = uniform_list("object-tree", rows.len(), move |range, _, cx| {
            view.update(cx, |this, cx| {
                range
                    .map(|index| {
                        let row = &rows[index];
                        let owner = row.owner;
                        let expanded = this.inspector.expanded.contains(&owner);
                        let st = this.state.borrow();
                        let active = Object::selected(&st) == owner;
                        let visible = st.object_visible(owner);
                        let icon = match owner {
                            Object::Video => ICON_VIDEO,
                            Object::Axes => ICON_AXES,
                            Object::Stick(_) => ICON_RULER,
                            Object::Track(id)
                                if st.tracks.iter().any(|t| t.id == id && !t.is_point_mass()) =>
                            {
                                ICON_RULER
                            }
                            _ => ICON_TRACKS,
                        };
                        div()
                            .id(("object-node", index))
                            .w_full()
                            .h(px(28.))
                            .flex()
                            .items_center()
                            .gap_1()
                            .pr_1()
                            .pl(px(4. + row.depth as f32 * 14.))
                            .bg(c(if active { 0x1b2b45 } else { 0x0d1420 }))
                            .text_xs()
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                                    window.focus(this.inspector.focus.as_ref().unwrap(), cx);
                                    this.state.borrow_mut().select_object(owner);
                                    if ev.click_count == 2 {
                                        if let Some(input) =
                                            this.begin_object_rename(owner, window, cx)
                                        {
                                            input.update(cx, |input, cx| input.focus(window, cx));
                                        }
                                    }
                                    crate::analysis::notify_workspace_windows(cx);
                                }),
                            )
                            .child(if owner == Object::Video {
                                div()
                                    .id("video-expand")
                                    .w(px(16.))
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            if !this.inspector.expanded.remove(&Object::Video) {
                                                this.inspector.expanded.insert(Object::Video);
                                            }
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    )
                                    .child(if expanded { "▾" } else { "▸" })
                                    .into_any_element()
                            } else {
                                div().w(px(16.)).into_any_element()
                            })
                            .child(
                                svg()
                                    .data(icon.as_bytes())
                                    .flex_none()
                                    .w(px(14.))
                                    .h(px(14.))
                                    .text_color(c(0x8fa3bf)),
                            )
                            .child(
                                if let Some((_, _, input)) = this
                                    .inspector
                                    .name
                                    .as_ref()
                                    .filter(|(_, object, _)| *object == owner)
                                {
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|_, _, _, cx| cx.stop_propagation()),
                                        )
                                        .capture_key_down(cx.listener(
                                            |this, event: &KeyDownEvent, window, cx| {
                                                if event.keystroke.key == "escape" {
                                                    this.inspector.name = None;
                                                    this.inspector.error = None;
                                                    window.focus(
                                                        this.inspector.focus.as_ref().unwrap(),
                                                        cx,
                                                    );
                                                    cx.stop_propagation();
                                                    cx.notify();
                                                }
                                            },
                                        ))
                                        .child(crate::widgets::property_input(
                                            "tree-object-name",
                                            input,
                                            cx,
                                        ))
                                        .into_any_element()
                                } else {
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_color(c(0xdbe6f5))
                                        .child(row.label.clone())
                                        .into_any_element()
                                },
                            )
                            .when(owner != Object::Video, |row| {
                                row.child(
                                    div()
                                        .id(("object-visible", index))
                                        .w(px(22.))
                                        .text_color(c(if visible { 0x8ecbff } else { 0x5f6f88 }))
                                        .cursor_pointer()
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                this.state
                                                    .borrow_mut()
                                                    .toggle_object_visible(owner);
                                                cx.stop_propagation();
                                                crate::analysis::notify_workspace_windows(cx);
                                            }),
                                        )
                                        .child(
                                            svg()
                                                .data(if visible {
                                                    ICON_EYE.as_bytes()
                                                } else {
                                                    ICON_EYE_OFF.as_bytes()
                                                })
                                                .w(px(14.))
                                                .h(px(14.))
                                                .text_color(c(if visible {
                                                    0x8ecbff
                                                } else {
                                                    0x5f6f88
                                                })),
                                        ),
                                )
                            })
                            .into_any_element()
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
        })
        .w_full()
        .min_w_0()
        .flex_1()
        .min_h_0()
        .track_scroll(&self.inspector.scroll);
        let tree = div()
            .track_focus(self.inspector.focus.as_ref().unwrap())
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                if ev.keystroke.key == "f2" {
                    let object = Object::selected(&this.state.borrow());
                    if let Some(input) = this.begin_object_rename(object, window, cx) {
                        input.update(cx, |input, cx| input.focus(window, cx));
                    }
                    cx.stop_propagation();
                }
            }))
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                div()
                    .flex_none()
                    .text_xs()
                    .text_color(c(0x7d8ca6))
                    .py_1()
                    .child(tr("inspector.objects")),
            )
            .when_some(self.inspector.error.clone(), |tree, error| {
                tree.child(div().text_xs().text_color(c(0xff9090)).child(error))
            })
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .vertical_scrollbar(&self.inspector.scroll.0.borrow().base_handle)
                    .child(list),
            );
        let title = self
            .state
            .borrow()
            .object_name(selected)
            .unwrap_or_default();
        let mut details = div()
            .id("selected-object-properties")
            .size_full()
            .min_h_0()
            .overflow_y_scrollbar()
            .flex()
            .flex_col()
            .gap_2()
            .py_2()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(0xb9cbe4))
                    .child(title),
            );
        details = details.children(if selected == Object::Video {
            video_rows
        } else {
            object_rows
        });
        if let Object::Track(id) = selected {
            let st = self.state.borrow();
            if let Some(track) = st
                .tracks
                .iter()
                .find(|t| t.id == id)
                .filter(|t| t.is_point_mass())
            {
                for var in [PlotVar::X, PlotVar::Y] {
                    let value = var.value(
                        track,
                        st.frame,
                        &st.coords,
                        st.video
                            .as_ref()
                            .map(|v| v.meta.frame_times.as_slice())
                            .unwrap_or(&[]),
                    );
                    details = details.child(kv(
                        var.axis_label_for(track, &st.calibration.unit),
                        value
                            .map(|v| format!("{:.*}", st.calibration.decimals, v))
                            .unwrap_or_else(|| "-".into()),
                    ));
                }
            }
        }
        if custom {
            details = details.child(self.variable_editor.as_ref().unwrap().clone());
        }
        if self.inspector.split.is_none() {
            self.inspector.split = Some(cx.new(|_| ResizableState::default()));
        }
        let group = v_resizable("object-inspector-split")
            .with_state(self.inspector.split.as_ref().unwrap())
            .child(
                resizable_panel()
                    .size(px(250.))
                    .size_range(px(90.)..Pixels::MAX)
                    .child(tree),
            )
            .child(
                resizable_panel()
                    .size(px(350.))
                    .size_range(px(130.)..Pixels::MAX)
                    .child(details),
            );
        div()
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_hidden()
            .child(group)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::gpui;
    use core::prelude::v1::test;
    #[test]
    fn tree_separates_objects_properties_and_selects_tracks_by_id() {
        let mut doc = DocState::placeholder();
        let mut a = Track::point_mass(7, "A", 0);
        let variable = a.set_variable(None, "r", "hypot(x,y)").unwrap();
        doc.tracks = vec![a, Track::measurement(42, MeasurementKind::Tape, "Tape", 0)];
        let mut expanded: HashSet<_> = [Object::Video].into();
        let collapsed = tree_rows(&doc, &expanded);
        assert_eq!(collapsed.len(), 5);
        assert!(collapsed.iter().any(|row| row.owner == Object::Track(42)));
        expanded.insert(Object::Track(7));
        let rows = tree_rows(&doc, &expanded);
        assert_eq!(
            rows.len(),
            collapsed.len(),
            "objects must not expose properties in the tree"
        );
        doc.select_object(Object::Track(7));
        assert!(doc.table_variables().contains(&PlotVar::Custom(variable)));
        doc.select_object(Object::Track(42));
        assert_eq!(doc.active, 1);
        assert!(doc.selection == Selection::Track(42));
        doc.select_object(Object::Video);
        assert!(doc.selection == Selection::None);
        expanded.remove(&Object::Video);
        assert_eq!(tree_rows(&doc, &expanded).len(), 1);
    }
    #[test]
    fn names_survive_project_serialization_and_undo_snapshots() {
        let mut doc = DocState::placeholder();
        doc.tracks.push(Track::point_mass(7, "A", 0));
        doc.calibration.add_extra_stick([0., 0.], [10., 0.], 1.);
        for (object, name) in [
            (Object::Axes, "坐标"),
            (Object::Stick(0), "基准尺"),
            (Object::Stick(1), "辅助尺"),
            (Object::Track(7), "物体甲"),
        ] {
            doc.rename_object(object, name).unwrap();
            assert_eq!(doc.object_name(object).as_deref(), Some(name));
        }
        let snapshot = doc.snapshot();
        doc.rename_object(Object::Axes, "changed").unwrap();
        doc.rename_object(Object::Track(7), "changed").unwrap();
        doc.restore_snapshot(&snapshot);
        assert_eq!(doc.coords.name, "坐标");
        assert_eq!(doc.tracks[0].name, "物体甲");
        let coords: ImageCoordSystem =
            serde_json::from_str(&serde_json::to_string(&doc.coords).unwrap()).unwrap();
        let calibration: Calibration =
            serde_json::from_str(&serde_json::to_string(&doc.calibration).unwrap()).unwrap();
        assert_eq!(coords.name, "坐标");
        assert_eq!(calibration.name, "基准尺");
        assert_eq!(calibration.extra_sticks[0].name, "辅助尺");
        assert!(doc.rename_object(Object::Track(7), "   ").is_err());
        assert_eq!(doc.tracks[0].name, "物体甲");
        doc.calibration.remove_stick(0);
        assert_eq!(doc.calibration.name, "辅助尺");
    }
    #[test]
    fn legacy_objects_load_without_names() {
        let mut value = serde_json::to_value(ImageCoordSystem::default()).unwrap();
        value.as_object_mut().unwrap().remove("name");
        let coords: ImageCoordSystem = serde_json::from_value(value).unwrap();
        assert!(coords.name.is_empty());
        let mut value = serde_json::to_value(Calibration::default()).unwrap();
        value.as_object_mut().unwrap().remove("name");
        let calibration: Calibration = serde_json::from_value(value).unwrap();
        assert!(calibration.name.is_empty());
    }
    struct InspectorHost(Entity<Probe>);
    impl Render for InspectorHost {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(self.0.update(cx, |probe, cx| {
                    probe.render_object_inspector(vec![], vec![], window, cx)
                }))
        }
    }
    #[gpui::test]
    fn native_inspector_draws_embedded_properties_and_commits_unicode_names(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_kit::init);
        let mut state = State::with_preferences(Preferences::default());
        let mut track = Track::point_mass(7, "A", 0);
        track.set_point(0, 3., 4.);
        track.set_variable(None, "r", "hypot(x,y)").unwrap();
        state.tracks.push(track);
        state.select_object(Object::Track(7));
        let original = state.snapshot();
        let state = Rc::new(RefCell::new(state));
        let probe = cx.new(|cx| {
            Probe::analysis_instance(
                state.clone(),
                None,
                Rc::new(RefCell::new(crate::analysis::PaneWindows::default())),
                cx,
            )
        });
        let (handle, host) = cx
            .update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds {
                            origin: point(px(0.), px(0.)),
                            size: size(px(400.), px(700.)),
                        })),
                        ..Default::default()
                    },
                    cx,
                    |_, cx| cx.new(|_| InspectorHost(probe.clone())),
                )
            })
            .unwrap();
        handle
            .update(cx, |_, window, cx| {
                probe.update(cx, |probe, cx| {
                    probe.begin_object_rename(Object::Track(7), window, cx);
                })
            })
            .unwrap();
        host.update(cx, |_, cx| cx.notify());
        handle
            .update(cx, |_, window, cx| {
                let _ = window.draw(cx);
            })
            .unwrap();
        let input = probe.update(cx, |probe, _| {
            assert!(probe.variable_editor.is_some());
            assert!(probe.inspector.split.is_some());
            probe.inspector.name.as_ref().unwrap().2.clone()
        });
        cx.update(|cx| {
            assert_eq!(
                input.read(cx).value().to_string(),
                "A",
                "rename cleared the original name"
            )
        });
        crate::widgets::PROPERTY_INPUT_BOUNDS.with(|map| {
            let bounds = map.borrow()[&input.entity_id()];
            assert!(
                f32::from(bounds.size.width) > 250.,
                "rename input collapsed to {:?}",
                bounds.size
            );
            assert!(
                f32::from(bounds.size.height) >= 20.,
                "rename input has no usable text height"
            );
        });
        handle
            .update(cx, |_, window, cx| {
                input.update(cx, |input, cx| {
                    input.set_value("追踪物体甲", window, cx);
                    cx.emit(InputEvent::PressEnter {
                        secondary: false,
                        shift: false,
                    });
                })
            })
            .unwrap();
        assert_eq!(state.borrow().tracks[0].name, "追踪物体甲");
        handle
            .update(cx, |_, window, cx| {
                let _ = window.draw(cx);
            })
            .unwrap();
        state.borrow_mut().restore_snapshot(&original);
        host.update(cx, |_, cx| cx.notify());
        handle
            .update(cx, |_, window, cx| {
                let _ = window.draw(cx);
            })
            .unwrap();
        probe.update(cx, |probe, _| assert!(probe.inspector.name.is_none()));
        assert_eq!(state.borrow().tracks[0].name, "A");
        state.borrow_mut().select_object(Object::Axes);
        host.update(cx, |_, cx| cx.notify());
        handle
            .update(cx, |_, window, cx| {
                let _ = window.draw(cx);
            })
            .unwrap();
        probe.update(cx, |probe, _| assert!(probe.inspector.name.is_none()));
        assert_eq!(state.borrow().tracks[0].variables.len(), 1);
        handle
            .update(cx, |_, window, _| window.activate_window())
            .unwrap();
        cx.run_until_parked();
        let rename = handle
            .update(cx, |_, window, cx| {
                probe.update(cx, |probe, cx| {
                    probe.begin_object_rename(Object::Axes, window, cx).unwrap()
                })
            })
            .unwrap();
        host.update(cx, |_, cx| cx.notify());
        handle
            .update(cx, |_, window, cx| {
                let _ = window.draw(cx);
            })
            .unwrap();
        handle
            .update(cx, |_, window, cx| {
                rename.update(cx, |input, cx| {
                    input.set_value("新坐标轴", window, cx);
                    input.focus(window, cx);
                })
            })
            .unwrap();
        // Record the focused input before testing its blur transition.
        handle
            .update(cx, |_, window, cx| {
                let _ = window.draw(cx);
            })
            .unwrap();
        handle.update(cx, |_, window, cx| window.blur(cx)).unwrap();
        handle
            .update(cx, |_, window, cx| {
                let _ = window.draw(cx);
            })
            .unwrap();
        assert_eq!(
            state.borrow().coords.name,
            "新坐标轴",
            "blur did not commit tree rename"
        );
        probe.update(cx, |probe, _| assert!(probe.inspector.name.is_none()));

        handle
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
    }
}

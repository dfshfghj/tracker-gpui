use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::i18n::tr;
use crate::*;

// Keep immutable series across scroll, selection and layout updates. Compare only
// numerical inputs, avoiding cloned rigid-body contours and other display state.
#[derive(Default)]
pub(crate) struct PlotCache(Option<CachedSeries>, Vec<Option<Entity<PlotLayer>>>);
struct CachedSeries {
    points: Vec<(usize, f32, f32)>,
    variables: Vec<crate::variables::Definition>,
    rigid_angles: Vec<(usize, f32)>,
    coords: crate::coords::ImageCoordSystem,
    times: Vec<f64>,
    axes: Vec<(PlotVar, PlotVar)>,
    series: Vec<Rc<PlotSeries>>,
}
impl PlotCache {
    fn series(
        &mut self,
        track: &Track,
        configs: &[PlotConfig],
        coords: &crate::coords::ImageCoordSystem,
        times: &[f64],
    ) -> Vec<Rc<PlotSeries>> {
        let axes: Vec<_> = configs.iter().map(|c| (c.x_var, c.y_var)).collect();
        let rigid_angles = || {
            track
                .rigid
                .iter()
                .flat_map(|rigid| rigid.steps.iter().map(|(f, step)| (*f, step.angle)))
        };
        let valid = self.0.as_ref().is_some_and(|cache| {
            cache.axes == axes
                && cache.coords == *coords
                && cache.times == times
                && cache.variables == track.variables
                && track.points().eq(cache.points.iter().copied())
                && rigid_angles().eq(cache.rigid_angles.iter().copied())
        });
        if !valid {
            self.1.truncate(configs.len());
            let series = configs
                .iter()
                .map(|config| {
                    let mut auto = config.clone();
                    auto.x_range = None;
                    auto.y_range = None;
                    Rc::new(plot::series(track, &auto, coords, times))
                })
                .collect();
            self.0 = Some(CachedSeries {
                points: track.points().collect(),
                variables: track.variables.clone(),
                rigid_angles: rigid_angles().collect(),
                coords: coords.clone(),
                times: times.to_vec(),
                axes,
                series,
            });
        }
        self.0.as_ref().unwrap().series.clone()
    }
}

// The immutable chart and the moving frame marker have different invalidation
// boundaries. A parent redraw must not invalidate this retained child view.
#[derive(Clone)]
struct PlotAppearance {
    series: Rc<PlotSeries>,
    x_range: (f64, f64),
    y_range: (f64, f64),
    labels: (String, String),
    color: u32,
    points: bool,
    lines: bool,
    light: bool,
    no_data: String,
}
impl PlotAppearance {
    fn same_as(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.series, &other.series)
            && self.x_range == other.x_range
            && self.y_range == other.y_range
            && self.labels == other.labels
            && self.color == other.color
            && self.points == other.points
            && self.lines == other.lines
            && self.light == other.light
            && self.no_data == other.no_data
    }
}
struct PlotLayer {
    appearance: PlotAppearance,
    geometry: Rc<RefCell<crate::plot_geometry::GeometryCache>>,
    #[cfg(test)]
    renders: usize,
    #[cfg(test)]
    paints: Rc<std::cell::Cell<usize>>,
}
impl PlotCache {
    fn layer(
        &mut self,
        index: usize,
        appearance: PlotAppearance,
        cx: &mut Context<Probe>,
    ) -> Entity<PlotLayer> {
        self.1.resize_with(self.1.len().max(index + 1), || None);
        if let Some(layer) = &self.1[index] {
            layer.update(cx, |layer, cx| {
                if !layer.appearance.same_as(&appearance) {
                    layer.appearance = appearance;
                    cx.notify();
                }
            });
        } else {
            self.1[index] = Some(cx.new(|_| PlotLayer {
                appearance,
                geometry: Rc::new(RefCell::new(crate::plot_geometry::GeometryCache::default())),
                #[cfg(test)]
                renders: 0,
                #[cfg(test)]
                paints: Rc::new(std::cell::Cell::new(0)),
            }));
        }
        self.1[index].as_ref().unwrap().clone()
    }
}
impl Render for PlotLayer {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        #[cfg(test)]
        {
            self.renders += 1;
        }
        let appearance = self.appearance.clone();
        let geometry = self.geometry.clone();
        #[cfg(test)]
        let paints = self.paints.clone();
        canvas(
            |_, _, _| (),
            move |bounds, _, window, cx| {
                #[cfg(test)]
                paints.set(paints.get() + 1);
                paint_plot_static(
                    bounds,
                    window,
                    cx,
                    &appearance.series,
                    &geometry,
                    appearance.x_range,
                    appearance.y_range,
                    &appearance.labels.0,
                    &appearance.labels.1,
                    appearance.color,
                    appearance.points,
                    appearance.lines,
                );
            },
        )
        .size_full()
    }
}

const PLOT_LEFT: f32 = 40.;
const PLOT_RIGHT: f32 = 10.;
const PLOT_TOP: f32 = 16.;
const PLOT_BOTTOM: f32 = 34.;
fn inner_plot_bounds(bounds: Bounds<Pixels>) -> Option<Bounds<Pixels>> {
    let width = f32::from(bounds.size.width) - PLOT_LEFT - PLOT_RIGHT;
    let height = f32::from(bounds.size.height) - PLOT_TOP - PLOT_BOTTOM;
    (width > 0. && height > 0.).then(|| Bounds {
        origin: point(
            bounds.origin.x + px(PLOT_LEFT),
            bounds.origin.y + px(PLOT_TOP),
        ),
        size: size(px(width), px(height)),
    })
}
fn paint_frame_marker(
    bounds: Bounds<Pixels>,
    window: &mut Window,
    series: &PlotSeries,
    x_range: (f64, f64),
    y_range: (f64, f64),
    frame: usize,
    color: u32,
) {
    if !bounds.intersects(&window.content_mask().bounds) {
        return;
    }
    let Some(bounds) = inner_plot_bounds(bounds) else {
        return;
    };
    if let Ok(index) = series.points.binary_search_by_key(&frame, |p| p.frame) {
        let viewport = crate::plot_geometry::Viewport {
            width: bounds.size.width.into(),
            height: bounds.size.height.into(),
            scale: window.scale_factor(),
            x_range,
            y_range,
        };
        let (x, y) = viewport.project(&series.points[index]);
        let (x, y) = (
            f32::from(bounds.origin.x) + x,
            f32::from(bounds.origin.y) + y,
        );
        if let Some(path) = circle_path(x, y, 5., true, 0.) {
            window.paint_path(path, c(0xffffff));
        }
        if let Some(path) = circle_path(x, y, 5., false, 2.) {
            window.paint_path(path, rgb(color));
        }
    }
}

pub(crate) struct PlotViewData {
    name: String,
    kind: String,
    configs: Vec<PlotConfig>,
    series: Vec<Rc<PlotSeries>>,
    shared_x: Option<(f64, f64)>,
    color: u32,
    axis_labels: Vec<(String, String)>,
    frame: usize,
    linked: bool,
    points_on: bool,
    lines_on: bool,
}

pub(crate) fn plot_sidebar(
    state: Rc<RefCell<State>>,
    only: Option<usize>,
    detached: Rc<RefCell<crate::analysis::PaneWindows>>,
    scroll: ScrollHandle,
    cache: &mut PlotCache,
    cx: &mut Context<Probe>,
) -> AnyElement {
    let prepared = {
        let st = state.borrow();
        let Some(track) = st.active_track().filter(|track| track.is_point_mass()) else {
            cache.0 = None;
            cache.1.clear();
            return div()
                .w_full()
                .text_xs()
                .text_color(c(0x5f6f88))
                .child(tr("plot.point_only"))
                .into_any_element();
        };
        let frame_times = st
            .video
            .as_ref()
            .map(|video| video.meta.frame_times.as_slice())
            .unwrap_or(&[]);
        let configs = st.plot.configs.clone();
        let series = cache.series(track, &configs, &st.coords, frame_times);
        let shared_x = if st.plot.linked_x && series.len() > 1 {
            let mut min = f64::INFINITY;
            let mut max = f64::NEG_INFINITY;
            for data in &series {
                min = min.min(data.x_range.0);
                max = max.max(data.x_range.1);
            }
            if min.is_finite() && max.is_finite() {
                Some((min, max))
            } else {
                None
            }
        } else {
            None
        };
        PlotViewData {
            name: track.name.clone(),
            kind: track.kind_label(),
            configs,
            series,
            shared_x,
            color: track.color,
            axis_labels: st
                .plot
                .configs
                .iter()
                .map(|config| {
                    (
                        config.x_var.axis_label_for(track, &st.calibration.unit),
                        config.y_var.axis_label_for(track, &st.calibration.unit),
                    )
                })
                .collect(),
            frame: st.frame,
            linked: st.plot.linked_x,
            points_on: st.plot.configs.iter().all(|config| config.points),
            lines_on: st.plot.configs.iter().all(|config| config.lines),
        }
    };

    let count = prepared.configs.len();
    let header = div()
        .flex()
        .flex_col()
        .gap_1()
        .flex_none()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .text_xs()
                        .text_color(c(0x7d8ca6))
                        .child(format!("{} · {}", prepared.name, prepared.kind)),
                )
                // A detached window shows exactly one plot, so the 1/2 switch
                // only makes sense in the sidebar.
                .when(only.is_none(), |this| {
                    this.child(plot_chip(0, tr("plot.one"), count == 1, cx, |this, cx| {
                        this.state.borrow_mut().set_plot_count(1);
                        crate::analysis::notify_workspace_windows(cx);
                    }))
                    .child(plot_chip(1, tr("plot.two"), count == 2, cx, |this, cx| {
                        this.state.borrow_mut().set_plot_count(2);
                        crate::analysis::notify_workspace_windows(cx);
                    }))
                }),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(check_item(
                    300,
                    tr("plot.points"),
                    prepared.points_on,
                    true,
                    cx,
                    |this, cx| {
                        this.state.borrow_mut().toggle_plot_points();
                        crate::analysis::notify_workspace_windows(cx);
                    },
                ))
                .child(check_item(
                    301,
                    tr("plot.lines"),
                    prepared.lines_on,
                    true,
                    cx,
                    |this, cx| {
                        this.state.borrow_mut().toggle_plot_lines();
                        crate::analysis::notify_workspace_windows(cx);
                    },
                ))
                .child(check_item(
                    302,
                    tr("plot.sync_x"),
                    prepared.linked,
                    true,
                    cx,
                    |this, cx| {
                        let mut st = this.state.borrow_mut();
                        st.plot.linked_x = !st.plot.linked_x;
                        drop(st);
                        crate::analysis::notify_workspace_windows(cx);
                    },
                ))
                .child(div().flex_1()),
        );

    let mut charts = div().flex().flex_col().gap_2();

    for (index, ((config, data), labels)) in prepared
        .configs
        .into_iter()
        .zip(prepared.series.into_iter())
        .zip(prepared.axis_labels.into_iter())
        .enumerate()
    {
        if only.is_some_and(|wanted| wanted != index) {
            continue;
        }
        if only.is_none()
            && detached
                .borrow()
                .windows
                .contains_key(&crate::analysis::Pane::Plot(index))
        {
            let pane = crate::analysis::Pane::Plot(index);
            charts =
                charts.child(
                    div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap_2()
                        .py_2()
                        .child(div().flex_1().text_xs().text_color(c(0x7d8ca6)).child(
                            rust_i18n::t!("analysis.plot_detached", n = index + 1).to_string(),
                        ))
                        .child(icon_button(
                            "show-plot-window",
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
                            "return-plot-window",
                            ICON_DOCK,
                            tr("analysis.dock"),
                            24.,
                            15.,
                            false,
                            true,
                            cx,
                            move |this, cx| this.dock_analysis_window(pane, cx),
                        )),
                );
            continue;
        }
        let mut card = div().w_full().min_h(px(160.)).flex();
        card = if only.is_some() {
            card.flex_1()
        } else {
            card.flex_none().h(px(200.))
        };
        charts = charts.child(card.child(plot_chart(
            cx,
            index,
            config,
            data,
            prepared.shared_x,
            prepared.color,
            labels,
            prepared.frame,
            only.is_some(),
            cache,
        )));
    }

    let charts = if only.is_some() {
        charts.flex_1().min_h_0().into_any_element()
    } else {
        crate::scroll_area::vertical_frame(
            "analysis-charts-scrollbar",
            div()
                .id("analysis-charts-scroll")
                .relative()
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&scroll)
                .child(charts),
            &scroll,
        )
        .into_any_element()
    };
    div()
        .w_full()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .gap_2()
        .child(header)
        .child(charts)
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn plot_chart(
    cx: &mut Context<Probe>,
    index: usize,
    config: PlotConfig,
    data: Rc<PlotSeries>,
    shared_x: Option<(f64, f64)>,
    color: u32,
    labels: (String, String),
    frame: usize,
    floating: bool,
    cache: &mut PlotCache,
) -> AnyElement {
    let x_range = shared_x.unwrap_or(data.x_range);
    let y_range = data.y_range;
    let points = data;
    let bounds_cell = Rc::new(std::cell::Cell::new((0.0f32, 0.0f32, 1.0f32, 1.0f32)));

    let x_chip = plot_var_chip(cx, index, PlotAxis::X, format!("{} ▾", labels.0));
    let y_chip = plot_var_chip(cx, index, PlotAxis::Y, format!("{} ▾", labels.1));

    let static_layer = cache.layer(
        index,
        PlotAppearance {
            series: points.clone(),
            x_range,
            y_range,
            labels: labels.clone(),
            color,
            points: config.points,
            lines: config.lines,
            light: crate::theme::light_theme(),
            no_data: tr("plot.no_data"),
        },
        cx,
    );
    let prepaint_bounds = bounds_cell.clone();
    let click_points = points.clone();
    let (x_min, x_max) = x_range;
    let marker_series = points.clone();
    let marker = canvas(
        move |bounds, _, _| {
            if let Some(bounds) = inner_plot_bounds(bounds) {
                prepaint_bounds.set((
                    bounds.origin.x.into(),
                    bounds.origin.y.into(),
                    bounds.size.width.into(),
                    bounds.size.height.into(),
                ));
            }
        },
        move |bounds, _, window, _| {
            paint_frame_marker(
                bounds,
                window,
                &marker_series,
                x_range,
                y_range,
                frame,
                color,
            )
        },
    )
    .absolute()
    .size_full();

    let click_bounds = bounds_cell.clone();
    div()
        .flex_1()
        .min_h(px(120.))
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .flex_none()
                .child(x_chip)
                .child(y_chip)
                .child(div().flex_1())
                .child(icon_button(
                    "detach-chart",
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
                            this.dock_analysis_window(crate::analysis::Pane::Plot(index), cx)
                        } else {
                            this.open_analysis_window(crate::analysis::Pane::Plot(index), cx)
                        }
                    },
                )),
        )
        .child(
            div()
                .relative()
                .flex_1()
                .min_h(px(80.))
                .rounded_sm()
                .border_1()
                .border_color(c(0x1d2a3d))
                .overflow_hidden()
                .bg(c(0x0a1018))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, ev: &MouseDownEvent, _window, cx| {
                        let (bx, by, bw, bh) = click_bounds.get();
                        let x = f32::from(ev.position.x);
                        let y = f32::from(ev.position.y);
                        if x < bx || y < by || x > bx + bw || y > by + bh {
                            return;
                        }
                        let fraction = ((x - bx) / bw.max(1.0)).clamp(0.0, 1.0) as f64;
                        let data_x = x_min + fraction * (x_max - x_min);
                        if let Some(point) = plot::nearest_point(&click_points.points, data_x) {
                            let mut st = this.state.borrow_mut();
                            st.pause();
                            st.set_frame(point.frame);
                            drop(st);
                            crate::analysis::notify_workspace_windows(cx);
                        }
                    }),
                )
                .child(static_layer.cached(StyleRefinement::default().absolute().size_full()))
                .child(marker),
        )
        .into_any_element()
}

fn paint_plot_static(
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
    series: &Rc<PlotSeries>,
    cache: &RefCell<crate::plot_geometry::GeometryCache>,
    x_range: (f64, f64),
    y_range: (f64, f64),
    x_axis: &str,
    y_axis: &str,
    color: u32,
    show_points: bool,
    show_lines: bool,
) {
    if !bounds.intersects(&window.content_mask().bounds) {
        return;
    }
    let ox = f32::from(bounds.origin.x);
    let oy = f32::from(bounds.origin.y);
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    if width < 40.0 || height < 40.0 {
        return;
    }
    let left = PLOT_LEFT;
    let right = width - PLOT_RIGHT;
    let top = PLOT_TOP;
    let bottom = height - PLOT_BOTTOM;
    if right <= left || bottom <= top {
        return;
    }
    let plot_width = right - left;
    let plot_height = bottom - top;
    let plot_bounds = Bounds {
        origin: point(bounds.origin.x + px(left), bounds.origin.y + px(top)),
        size: size(px(plot_width), px(plot_height)),
    };
    window.paint_quad(quad(
        plot_bounds,
        px(0.),
        c(0x0d1420),
        px(0.),
        transparent_black(),
        Default::default(),
    ));

    let (x_min, x_max) = x_range;
    let (y_min, y_max) = y_range;
    let x_span = (x_max - x_min).max(1e-12);
    let y_span = (y_max - y_min).max(1e-12);
    let x_ticks = plot::nice_ticks(x_min, x_max, 6);
    let y_ticks = plot::nice_ticks(y_min, y_max, 5);
    let grid_color = c(0x1d2a3d).alpha(0.75);
    let axis_color = c(0x5f6f88).alpha(0.8);

    for tick in &x_ticks {
        let x = left + ((tick - x_min) / x_span) as f32 * plot_width;
        if let Some(path) = stroke_path(&[(ox + x, oy + top), (ox + x, oy + bottom)], 1.0) {
            window.paint_path(path, grid_color);
        }
    }
    for tick in &y_ticks {
        let y = bottom - ((tick - y_min) / y_span) as f32 * plot_height;
        if let Some(path) = stroke_path(&[(ox + left, oy + y), (ox + right, oy + y)], 1.0) {
            window.paint_path(path, grid_color);
        }
    }

    if x_min <= 0.0 && x_max >= 0.0 {
        let x = left + ((0.0 - x_min) / x_span) as f32 * plot_width;
        if let Some(path) = stroke_path(&[(ox + x, oy + top), (ox + x, oy + bottom)], 1.2) {
            window.paint_path(path, axis_color);
        }
    }
    if y_min <= 0.0 && y_max >= 0.0 {
        let y = bottom - ((0.0 - y_min) / y_span) as f32 * plot_height;
        if let Some(path) = stroke_path(&[(ox + left, oy + y), (ox + right, oy + y)], 1.2) {
            window.paint_path(path, axis_color);
        }
    }

    let step_x = x_ticks
        .windows(2)
        .next()
        .map(|pair| pair[1] - pair[0])
        .unwrap_or(1.0);
    let step_y = y_ticks
        .windows(2)
        .next()
        .map(|pair| pair[1] - pair[0])
        .unwrap_or(1.0);
    let tick_color = c(0x7d8ca6);
    for tick in &x_ticks {
        let x = left + ((tick - x_min) / x_span) as f32 * plot_width;
        if x < left - 1.0 || x > right + 1.0 {
            continue;
        }
        if let Some(path) = stroke_path(&[(ox + x, oy + bottom), (ox + x, oy + bottom + 4.0)], 1.0)
        {
            window.paint_path(path, axis_color);
        }
        plot_text(
            window,
            cx,
            &format_tick(*tick, step_x),
            f32::from(bounds.origin.x) + x - 10.0,
            f32::from(bounds.origin.y) + height - 27.0,
            9.0,
            tick_color,
        );
    }
    for tick in &y_ticks {
        let y = bottom - ((tick - y_min) / y_span) as f32 * plot_height;
        if y < top - 1.0 || y > bottom + 1.0 {
            continue;
        }
        if let Some(path) = stroke_path(&[(ox + left - 4.0, oy + y), (ox + left, oy + y)], 1.0) {
            window.paint_path(path, axis_color);
        }
        let text = format_tick(*tick, step_y);
        let label_x = left - 6.0 - text.len() as f32 * 5.5;
        plot_text(
            window,
            cx,
            &text,
            f32::from(bounds.origin.x) + label_x,
            f32::from(bounds.origin.y) + y - 6.0,
            9.0,
            tick_color,
        );
    }

    plot_text(
        window,
        cx,
        y_axis.to_string(),
        f32::from(bounds.origin.x) + left + 2.0,
        f32::from(bounds.origin.y) + 1.0,
        10.0,
        c(0xdbe6f5),
    );
    let x_label_x = right - x_axis.len() as f32 * 5.5;
    plot_text(
        window,
        cx,
        x_axis.to_string(),
        f32::from(bounds.origin.x) + x_label_x,
        f32::from(bounds.origin.y) + height - 12.0,
        10.0,
        c(0xdbe6f5),
    );

    let points = &series.points;
    if points.is_empty() {
        plot_text(
            window,
            cx,
            tr("plot.no_data"),
            f32::from(bounds.origin.x) + left + plot_width * 0.5 - 22.0,
            f32::from(bounds.origin.y) + top + plot_height * 0.5 - 6.0,
            11.0,
            c(0x5f6f88),
        );
        return;
    }

    let series_color = rgb(color);
    let viewport = crate::plot_geometry::Viewport {
        width: plot_width,
        height: plot_height,
        scale: window.scale_factor(),
        x_range,
        y_range,
    };
    let geometry = cache.borrow_mut().get(series, viewport);
    if show_lines && geometry.positions.len() > 1 {
        if let Some(path) = geometry.line_path(point(px(ox + left), px(oy + top))) {
            window.paint_path(path, series_color.alpha(0.9));
        }
    }
    if show_points {
        for &(x, y) in &geometry.positions {
            // A rounded quad draws a circle in the GPU shader, avoiding one
            // 32-segment tessellated path per marker.
            window.paint_quad(quad(
                Bounds {
                    origin: point(px(ox + left + x - 2.3), px(oy + top + y - 2.3)),
                    size: size(px(4.6), px(4.6)),
                },
                px(2.3),
                series_color.alpha(0.85),
                px(0.),
                transparent_black(),
                Default::default(),
            ));
        }
    }
}

pub(crate) fn format_tick(value: f64, step: f64) -> String {
    let decimals = if step >= 1.0 {
        0
    } else {
        (-step.abs().log10()).ceil() as usize
    };
    format!("{:.*}", decimals.min(5), value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::gpui;
    use core::prelude::v1::test;

    #[test]
    fn plot_cache_reuses_series_and_invalidates_each_numerical_input() {
        let mut cache = PlotCache::default();
        let mut track = Track::point_mass(1, "Track", 0);
        track.set_point(0, 2., 3.);
        track.set_point(1, 5., 7.);
        let id = track.set_variable(None, "derived", "x*2").unwrap();
        let mut configs = vec![PlotConfig {
            x_var: PlotVar::Time,
            y_var: PlotVar::Custom(id),
            ..PlotConfig::default()
        }];
        let mut coords = crate::coords::ImageCoordSystem::default();
        let mut times = vec![0., 1.];
        let mut previous = cache.series(&track, &configs, &coords, &times);
        assert!(Rc::ptr_eq(
            &previous[0],
            &cache.series(&track, &configs, &coords, &times)[0]
        ));
        // Cosmetic changes should not recalculate values.
        configs[0].points = false;
        track.name = "Renamed".into();
        assert!(Rc::ptr_eq(
            &previous[0],
            &cache.series(&track, &configs, &coords, &times)[0]
        ));
        for input in 0..5 {
            match input {
                0 => track.set_point(1, 8., 9.),
                1 => coords.base.scale = [2., 2.],
                2 => times[1] = 2.,
                3 => {
                    track.set_variable(Some(id), "derived", "x*3").unwrap();
                }
                _ => configs[0].y_var = PlotVar::Y,
            }
            let next = cache.series(&track, &configs, &coords, &times);
            assert!(
                !Rc::ptr_eq(&previous[0], &next[0]),
                "stale series for input {input}"
            );
            let expected = plot::series(&track, &configs[0], &coords, &times);
            assert_eq!(next[0].points.len(), expected.points.len());
            for (a, b) in next[0].points.iter().zip(&expected.points) {
                assert_eq!((a.frame, a.x, a.y), (b.frame, b.x, b.y));
            }
            previous = next;
        }
        track.delete_point(1);
        assert_eq!(
            cache.series(&track, &configs, &coords, &times)[0]
                .points
                .len(),
            1
        );
    }
    #[test]
    fn cached_rigid_angles_update_without_center_motion() {
        let mut cache = PlotCache::default();
        let mut track = Track::rigid_body(1, "Rigid", 0, "MOTION", vec![]);
        track.set_rigid_step(0, 1., 2., 0.1, 10., 1);
        let coords = crate::coords::ImageCoordSystem::default();
        let configs = [PlotConfig {
            x_var: PlotVar::Time,
            y_var: PlotVar::Angle,
            ..PlotConfig::default()
        }];
        let before = cache.series(&track, &configs, &coords, &[0.]);
        track.set_rigid_angle(0, 0.5);
        let after = cache.series(&track, &configs, &coords, &[0.]);
        assert!(!Rc::ptr_eq(&before[0], &after[0]));
        assert_ne!(before[0].points[0].y, after[0].points[0].y);
    }
    #[gpui::test]
    fn frame_updates_reuse_static_layer_and_data_changes_invalidate_it(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let mut state = State::with_preferences(Preferences::default());
        let mut track = Track::point_mass(1, "Track", 0);
        for frame in 0..10_000 {
            track.set_point(frame, frame as f32, 1.);
        }
        state.tracks.push(track);
        state.plot.configs = vec![PlotConfig {
            x_var: PlotVar::Frame,
            y_var: PlotVar::X,
            ..PlotConfig::default()
        }];
        let state = Rc::new(RefCell::new(state));
        let registry = Rc::new(RefCell::new(crate::analysis::PaneWindows::default()));
        let owner =
            cx.new(|cx| Probe::analysis_instance(state.clone(), None, registry.clone(), cx));
        owner.update(cx, |view, cx| {
            view.open_analysis_window(crate::analysis::Pane::Plot(0), cx)
        });
        let handle = registry.borrow().windows[&crate::analysis::Pane::Plot(0)];
        let view = handle
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
        let layer = view.update(cx, |view, _| view.plot_cache.1[0].as_ref().unwrap().clone());
        let counts = |cx: &mut TestAppContext| {
            layer.update(cx, |layer, _| (layer.renders, layer.paints.get()))
        };
        let baseline = counts(cx);
        assert!(baseline.0 > 0 && baseline.1 > 0);
        for frame in 1..4 {
            view.update(cx, |view, cx| {
                view.state.borrow_mut().frame = frame;
                crate::analysis::notify_workspace_windows(cx);
            });
            handle
                .update(cx, |_, window, cx| {
                    let _ = window.draw(cx);
                })
                .unwrap();
            assert_eq!(
                counts(cx),
                baseline,
                "frame {frame} repainted the static plot"
            );
        }
        view.update(cx, |view, cx| {
            view.state.borrow_mut().tracks[0].set_point(5, 9., 2.);
            cx.notify();
        });
        handle
            .update(cx, |_, window, cx| {
                let _ = window.draw(cx);
            })
            .unwrap();
        let after = counts(cx);
        assert!(
            after.0 > baseline.0 && after.1 > baseline.1,
            "edited data did not invalidate the static plot"
        );
        // Cosmetic inputs also invalidate the cached static view.
        view.update(cx, |view, cx| {
            view.state.borrow_mut().tracks[0].color = 0xff0000;
            cx.notify();
        });
        handle
            .update(cx, |_, window, cx| {
                let _ = window.draw(cx);
            })
            .unwrap();
        assert!(
            counts(cx).1 > after.1,
            "changed color was cached indefinitely"
        );
        handle
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
    }
}

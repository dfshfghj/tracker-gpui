use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::*;

use crate::*;

pub(crate) struct PlotViewData {
    name: String,
    kind: &'static str,
    configs: Vec<PlotConfig>,
    series: Vec<PlotSeries>,
    shared_x: Option<(f64, f64)>,
    color: u32,
    unit: String,
    frame: usize,
    linked: bool,
    points_on: bool,
    lines_on: bool,
}

pub(crate) fn plot_sidebar(state: Rc<RefCell<State>>, cx: &mut Context<Probe>) -> AnyElement {
    let prepared = {
        let st = state.borrow();
        let Some(track) = st.active_track().filter(|track| track.is_point_mass()) else {
            return div()
                .w_full()
                .text_xs()
                .text_color(c(0x5f6f88))
                .child("绘图仅用于质点轨道")
                .into_any_element();
        };
        let frame_times = st
            .video
            .as_ref()
            .map(|video| video.meta.frame_times.as_slice())
            .unwrap_or(&[]);
        let configs = st.plot.configs.clone();
        let series: Vec<PlotSeries> = configs
            .iter()
            .map(|config| {
                let mut auto = config.clone();
                auto.x_range = None;
                auto.y_range = None;
                plot::series(track, &auto, &st.coords, frame_times)
            })
            .collect();
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
            unit: st.calibration.unit.clone(),
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
                .child(plot_chip(0, "1 图".into(), count == 1, cx, |this, _cx| {
                    this.state.borrow_mut().set_plot_count(1);
                }))
                .child(plot_chip(1, "2 图".into(), count == 2, cx, |this, _cx| {
                    this.state.borrow_mut().set_plot_count(2);
                })),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(check_item(
                    300,
                    "点",
                    prepared.points_on,
                    true,
                    cx,
                    |this, _cx| {
                        this.state.borrow_mut().toggle_plot_points();
                    },
                ))
                .child(check_item(
                    301,
                    "线",
                    prepared.lines_on,
                    true,
                    cx,
                    |this, _cx| {
                        this.state.borrow_mut().toggle_plot_lines();
                    },
                ))
                .child(check_item(
                    302,
                    "同步 X",
                    prepared.linked,
                    true,
                    cx,
                    |this, _cx| {
                        let mut st = this.state.borrow_mut();
                        st.plot.linked_x = !st.plot.linked_x;
                    },
                ))
                .child(div().flex_1()),
        );

    let mut charts = div().flex().flex_col().gap_2();
    let unit = prepared.unit.clone();
    for (index, (config, data)) in prepared
        .configs
        .into_iter()
        .zip(prepared.series.into_iter())
        .enumerate()
    {
        charts = charts.child(div().w_full().h(px(200.)).flex().child(plot_chart(
            cx,
            index,
            config,
            data,
            prepared.shared_x,
            prepared.color,
            unit.clone(),
            prepared.frame,
        )));
    }

    div()
        .w_full()
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
    data: PlotSeries,
    shared_x: Option<(f64, f64)>,
    color: u32,
    unit: String,
    frame: usize,
) -> AnyElement {
    let x_range = shared_x.unwrap_or(data.x_range);
    let y_range = data.y_range;
    let points = Rc::new(data.points);
    let bounds_cell = Rc::new(std::cell::Cell::new((0.0f32, 0.0f32, 1.0f32, 1.0f32)));

    let x_chip = plot_var_chip(
        cx,
        index,
        PlotAxis::X,
        format!("{} ▾", config.x_var.axis_label(&unit)),
    );
    let y_chip = plot_var_chip(
        cx,
        index,
        PlotAxis::Y,
        format!("{} ▾", config.y_var.axis_label(&unit)),
    );

    let prepaint_bounds = bounds_cell.clone();
    let click_points = points.clone();
    let (x_min, x_max) = x_range;

    let canvas = canvas(
        move |bounds, _window, _cx| {
            prepaint_bounds.set((
                f32::from(bounds.origin.x),
                f32::from(bounds.origin.y),
                f32::from(bounds.size.width),
                f32::from(bounds.size.height),
            ));
        },
        {
            let points = points.clone();
            let unit = unit.clone();
            let x_var = config.x_var;
            let y_var = config.y_var;
            move |bounds, _prepaint, window, cx| {
                paint_plot(
                    bounds,
                    window,
                    cx,
                    &points,
                    x_range,
                    y_range,
                    x_var,
                    y_var,
                    &unit,
                    color,
                    frame,
                    config.points,
                    config.lines,
                );
            }
        },
    )
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
                .child(
                    div()
                        .text_xs()
                        .text_color(c(0x5f6f88))
                        .child(format!("{} 点", points.len())),
                ),
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
                        if let Some(point) = plot::nearest_point(&click_points, data_x) {
                            let mut st = this.state.borrow_mut();
                            st.pause();
                            st.set_frame(point.frame);
                            drop(st);
                            cx.notify();
                        }
                    }),
                )
                .child(canvas),
        )
        .into_any_element()
}

pub(crate) fn paint_plot(
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
    points: &[PlotPoint],
    x_range: (f64, f64),
    y_range: (f64, f64),
    x_var: PlotVar,
    y_var: PlotVar,
    unit: &str,
    color: u32,
    frame: usize,
    show_points: bool,
    show_lines: bool,
) {
    let ox = f32::from(bounds.origin.x);
    let oy = f32::from(bounds.origin.y);
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    if width < 40.0 || height < 40.0 {
        return;
    }
    let left = 40.0;
    let right = width - 10.0;
    let top = 16.0;
    let bottom = height - 34.0;
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

    let x_axis = x_var.axis_label(unit);
    let y_axis = y_var.axis_label(unit);
    plot_text(
        window,
        cx,
        &y_axis,
        f32::from(bounds.origin.x) + left + 2.0,
        f32::from(bounds.origin.y) + 1.0,
        10.0,
        c(0xdbe6f5),
    );
    let x_label_x = right - x_axis.len() as f32 * 5.5;
    plot_text(
        window,
        cx,
        &x_axis,
        f32::from(bounds.origin.x) + x_label_x,
        f32::from(bounds.origin.y) + height - 12.0,
        10.0,
        c(0xdbe6f5),
    );

    if points.is_empty() {
        plot_text(
            window,
            cx,
            "暂无数据",
            f32::from(bounds.origin.x) + left + plot_width * 0.5 - 22.0,
            f32::from(bounds.origin.y) + top + plot_height * 0.5 - 6.0,
            11.0,
            c(0x5f6f88),
        );
        return;
    }

    let series_color = rgb(color);
    let screen: Vec<(f32, f32)> = points
        .iter()
        .map(|point| {
            let x = left + ((point.x - x_min) / x_span) as f32 * plot_width;
            let y = bottom - ((point.y - y_min) / y_span) as f32 * plot_height;
            (ox + x.clamp(left, right), oy + y.clamp(top, bottom))
        })
        .collect();

    if show_lines && screen.len() > 1 {
        if let Some(path) = stroke_path(&screen, 1.5) {
            window.paint_path(path, series_color.alpha(0.9));
        }
    }
    if show_points && screen.len() <= 3000 {
        for (index, point) in points.iter().enumerate() {
            let (x, y) = screen[index];
            if point.frame == frame {
                continue;
            }
            if let Some(path) = circle_path(x, y, 2.3, true, 0.0) {
                window.paint_path(path, series_color.alpha(0.85));
            }
        }
    }
    if let Some((index, _)) = points
        .iter()
        .enumerate()
        .find(|(_, point)| point.frame == frame)
    {
        let (x, y) = screen[index];
        if let Some(path) = circle_path(x, y, 5.0, true, 0.0) {
            window.paint_path(path, c(0xffffff));
        }
        if let Some(path) = circle_path(x, y, 5.0, false, 2.0) {
            window.paint_path(path, series_color);
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

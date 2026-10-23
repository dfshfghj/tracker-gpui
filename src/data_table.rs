use gpui_component::scroll::ScrollableElement;
use gpui_kit::*;

use crate::plot::PlotVar;
use crate::{c, i18n::tr, DocState, Probe};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Cell {
    row: usize,
    col: usize,
}

// Like JTable, selection is the Cartesian product of independent row/column sets.
// Disjoint intervals are allowed, but every exported row has exactly the same columns.
#[derive(Clone, Default)]
struct MatrixSelection {
    rows: std::collections::BTreeSet<usize>,
    cols: std::collections::BTreeSet<usize>,
}

impl MatrixSelection {
    fn contains(&self, cell: Cell) -> bool {
        self.rows.contains(&cell.row) && self.cols.contains(&cell.col)
    }

    fn is_empty(&self) -> bool {
        self.rows.is_empty() || self.cols.is_empty()
    }

    fn interval(&mut self, anchor: Cell, cell: Cell, remove: bool) {
        for row in anchor.row.min(cell.row)..=anchor.row.max(cell.row) {
            if remove {
                self.rows.remove(&row);
            } else {
                self.rows.insert(row);
            }
        }
        for col in anchor.col.min(cell.col)..=anchor.col.max(cell.col) {
            if remove {
                self.cols.remove(&col);
            } else {
                self.cols.insert(col);
            }
        }
    }
}

struct DragSelection {
    base: MatrixSelection,
    remove: bool,
}

#[derive(Default)]
pub(crate) struct TableSelection {
    context: Option<(u64, Vec<PlotVar>, Vec<usize>)>,
    selected: MatrixSelection,
    anchor: Cell,
    cursor: Cell,
    drag: Option<DragSelection>,
}

impl TableSelection {
    pub(crate) fn sync(&mut self, track: u64, columns: Vec<PlotVar>, frames: Vec<usize>) {
        let context = (track, columns, frames);
        if self.context.as_ref() != Some(&context) {
            self.clear();
            self.context = Some(context);
        }
    }

    pub(crate) fn clear(&mut self) {
        self.selected = MatrixSelection::default();
        self.anchor = Cell::default();
        self.cursor = Cell::default();
        self.drag = None;
    }

    fn begin(&mut self, cell: Cell, shift: bool, control: bool) {
        // Ctrl toggles the target's row and column, while Shift replaces selection
        // with an interval from the anchor. Ctrl+Shift adds/removes that interval.
        let remove = control
            && if shift {
                !self.selected.contains(self.anchor)
            } else {
                self.selected.contains(cell)
            };
        if !shift {
            self.anchor = cell;
        }
        self.drag = Some(DragSelection {
            base: if control {
                self.selected.clone()
            } else {
                MatrixSelection::default()
            },
            remove,
        });
        self.extend(cell);
    }

    fn extend(&mut self, cell: Cell) {
        self.cursor = cell;
        let (mut selected, remove) = self
            .drag
            .as_ref()
            .map(|drag| (drag.base.clone(), drag.remove))
            .unwrap_or_default();
        selected.interval(self.anchor, cell, remove);
        self.selected = selected;
    }

    pub(crate) fn select_all(&mut self, rows: usize, cols: usize) {
        self.clear();
        if rows > 0 && cols > 0 {
            self.anchor = Cell::default();
            self.extend(Cell {
                row: rows - 1,
                col: cols - 1,
            });
        }
    }

    fn select_column(&mut self, col: usize, rows: usize, shift: bool, control: bool) {
        if rows == 0 {
            return;
        }
        self.drag = None;
        if control {
            if !self.selected.cols.remove(&col) {
                self.selected.cols.insert(col);
            }
            if self.selected.rows.is_empty() {
                self.selected.rows.extend(0..rows);
            }
        } else if shift && !self.selected.is_empty() {
            self.selected.cols = (self.anchor.col.min(col)..=self.anchor.col.max(col)).collect();
        } else {
            self.selected.rows = (0..rows).collect();
            self.selected.cols = [col].into();
        }
        if !shift {
            self.anchor.col = col;
        }
        self.cursor.col = col;
    }

    fn navigate(&mut self, key: &str, shift: bool, control: bool, rows: usize, cols: usize) {
        if rows == 0 || cols == 0 {
            return;
        }
        let mut cell = if !self.selected.is_empty() {
            self.cursor
        } else {
            Cell::default()
        };
        match key {
            "up" => {
                cell.row = if control {
                    0
                } else {
                    cell.row.saturating_sub(1)
                }
            }
            "down" => {
                cell.row = if control {
                    rows - 1
                } else {
                    (cell.row + 1).min(rows - 1)
                }
            }
            "left" => {
                cell.col = if control {
                    0
                } else {
                    cell.col.saturating_sub(1)
                }
            }
            "right" => {
                cell.col = if control {
                    cols - 1
                } else {
                    (cell.col + 1).min(cols - 1)
                }
            }
            _ => return,
        }
        self.begin(cell, shift, false);
        self.drag = None;
    }

    fn contains(&self, cell: Cell) -> bool {
        self.selected.contains(cell)
    }

    #[cfg(test)]
    pub(crate) fn selected_rows(&self) -> usize {
        if self.selected.is_empty() {
            0
        } else {
            self.selected.rows.len()
        }
    }
}

pub(crate) struct TableData {
    headers: Vec<String>,
    frames: Vec<usize>,
    rows: Vec<Vec<String>>,
}

impl TableData {
    fn csv(&self, selection: &TableSelection) -> String {
        self.delimited(selection, ',')
    }

    fn delimited(&self, selection: &TableSelection, delimiter: char) -> String {
        if self.headers.is_empty() || self.rows.is_empty() {
            return String::new();
        }
        let all = selection.selected.is_empty();
        let columns: Vec<usize> = (0..self.headers.len())
            .filter(|col| all || selection.selected.cols.contains(col))
            .collect();
        let mut text = String::new();
        // Preserve display order while omitting unselected gaps, just like TrackerIO.getData.
        for row in std::iter::once(&self.headers).chain(
            self.rows
                .iter()
                .enumerate()
                .filter(|(index, _)| all || selection.selected.rows.contains(index))
                .map(|(_, row)| row),
        ) {
            for (index, &col) in columns.iter().enumerate() {
                if index > 0 {
                    text.push(delimiter);
                }
                let value = &row[col];
                if value.contains([delimiter, '"', '\r', '\n']) {
                    text.push('"');
                    text.push_str(&value.replace('"', "\"\""));
                    text.push('"');
                } else {
                    text.push_str(value);
                }
            }
            text.push_str("\r\n");
        }
        text
    }
}

impl crate::State {
    pub(crate) fn copy_table_to_clipboard(&mut self, cx: &mut App) {
        self.sync_table_selection();
        let text = self.table_data().delimited(
            &self.table_selection,
            self.prefs.table_copy_format.delimiter(),
        );
        if !text.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            self.status = Some(tr("status.table_copied"));
        }
    }
}

impl DocState {
    pub(crate) fn table_variables(&self) -> Vec<PlotVar> {
        [
            PlotVar::Time,
            PlotVar::X,
            PlotVar::Y,
            PlotVar::Vx,
            PlotVar::Vy,
        ]
        .into_iter()
        .enumerate()
        .filter(|(i, _)| self.table_columns[*i])
        .map(|(_, var)| var)
        .chain(
            self.active_track()
                .into_iter()
                .flat_map(|track| track.variables.iter())
                .filter(|def| def.table_visible)
                .map(|def| PlotVar::Custom(def.id)),
        )
        .collect()
    }

    pub(crate) fn table_data(&self) -> TableData {
        self.table_data_range(0..usize::MAX)
    }

    // The UI requests only visible rows; copying explicitly requests the full matrix.
    fn table_data_range(&self, range: std::ops::Range<usize>) -> TableData {
        let frames = self
            .active_track()
            .into_iter()
            .filter(|t| t.is_point_mass())
            .flat_map(|track| track.points().map(|(frame, _, _)| frame))
            .skip(range.start)
            .take(range.len());
        self.table_data_frames(frames)
    }

    fn table_data_frames(&self, frames: impl Iterator<Item = usize>) -> TableData {
        let columns = self.table_variables();
        let mut data = TableData {
            headers: Vec::new(),
            frames: Vec::new(),
            rows: Vec::new(),
        };
        if let Some(track) = self.active_track().filter(|t| t.is_point_mass()) {
            let vars = crate::variables::Variables::compile(track).ok();
            data.headers = columns
                .iter()
                .map(|var| {
                    vars.as_ref().map_or_else(
                        || var.axis_label_for(track, &self.calibration.unit),
                        |vars| vars.axis_label(*var, track, &self.calibration.unit),
                    )
                })
                .collect();
            let times = self
                .video
                .as_ref()
                .map(|v| v.meta.frame_times.as_slice())
                .unwrap_or(&[]);
            for frame in frames {
                data.frames.push(frame);
                data.rows.push(
                    columns
                        .iter()
                        .map(|var| {
                            let value = if *var == PlotVar::Time {
                                Some(self.frame_time(frame))
                            } else if let Some(vars) = &vars {
                                vars.value(*var, track, frame, &self.coords, times)
                            } else if !matches!(var, PlotVar::Custom(_)) {
                                var.value(track, frame, &self.coords, times)
                            } else {
                                None
                            };
                            value
                                .filter(|v| v.is_finite())
                                .map(|v| {
                                    format!(
                                        "{:.*}",
                                        if *var == PlotVar::Time {
                                            3
                                        } else {
                                            self.calibration.decimals
                                        },
                                        v
                                    )
                                })
                                .unwrap_or_else(|| "-".into())
                        })
                        .collect(),
                );
            }
        }
        data
    }

    pub(crate) fn sync_table_selection(&mut self) {
        let track = self.active_track().filter(|t| t.is_point_mass());
        let id = track.map_or(0, |t| t.id);
        let frames = track
            .map(|t| t.points().map(|(frame, _, _)| frame).collect())
            .unwrap_or_default();
        let columns = self.table_variables();
        self.table_selection.sync(id, columns, frames);
    }

    pub(crate) fn select_all_table(&mut self) {
        self.sync_table_selection();
        let rows = self
            .active_track()
            .filter(|t| t.is_point_mass())
            .map_or(0, |t| t.step_count());
        self.table_selection
            .select_all(rows, self.table_variables().len());
    }

    pub(crate) fn table_csv(&self) -> String {
        self.table_data().csv(&self.table_selection)
    }
}

pub(crate) fn table_view(
    st: &DocState,
    focus: &FocusHandle,
    scroll: &UniformListScrollHandle,
    horizontal: &ScrollHandle,
    cx: &mut Context<Probe>,
) -> AnyElement {
    let data = st.table_data_range(0..0);
    let min_width = px(data.headers.len() as f32 * 72.);
    let mut table = div()
        .id("data-table")
        .relative()
        .w_full()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .overflow_x_scroll()
        .track_scroll(horizontal)
        .horizontal_scrollbar(horizontal)
        .flex()
        .flex_col()
        .track_focus(focus)
        .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
            let mut st = this.state.borrow_mut();
            st.sync_table_selection();
            let control = ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform;
            match ev.keystroke.key.as_str() {
                "c" if control => {
                    st.copy_table_to_clipboard(cx);
                }
                "a" if control => st.select_all_table(),
                "escape" => st.table_selection.clear(),
                key @ ("left" | "right" | "up" | "down") => {
                    let rows = st
                        .active_track()
                        .filter(|t| t.is_point_mass())
                        .map_or(0, |t| t.step_count());
                    let cols = st.table_variables().len();
                    st.table_selection.navigate(
                        key,
                        ev.keystroke.modifiers.shift,
                        control,
                        rows,
                        cols,
                    );
                    this.table_scroll
                        .scroll_to_item(st.table_selection.cursor.row, ScrollStrategy::Nearest);
                }
                _ => return,
            }
            drop(st);
            cx.stop_propagation();
            crate::analysis::notify_workspace_windows(cx);
        }))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _, _, _| this.state.borrow_mut().table_selection.drag = None),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            cx.listener(|this, _, _, _| this.state.borrow_mut().table_selection.drag = None),
        );
    let mut header = div()
        .flex()
        .w_full()
        .min_w(min_width)
        .bg(c(0x111a29))
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(0x8fa3bf));
    let row_count = st
        .active_track()
        .filter(|t| t.is_point_mass())
        .map_or(0, |t| t.step_count());
    for (col, label) in data.headers.iter().enumerate() {
        header = header.child(
            div()
                .id(("table-column", col))
                .flex_1()
                .min_w_0()
                .px_2()
                .py_1()
                .border_1()
                .border_color(c(0x1d2a3d))
                .overflow_hidden()
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                        window.focus(&this.table_focus, cx);
                        let mut st = this.state.borrow_mut();
                        st.sync_table_selection();
                        st.table_selection.select_column(
                            col,
                            row_count,
                            ev.modifiers.shift,
                            ev.modifiers.control || ev.modifiers.platform,
                        );
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
                .child(label.clone()),
        );
    }
    table = table.child(header.flex_none());
    let frames: Vec<_> = st
        .active_track()
        .into_iter()
        .flat_map(|track| track.points().map(|(frame, _, _)| frame))
        .collect();
    let view = cx.entity().downgrade();
    let body = uniform_list("data-table-rows", row_count, move |range, _, cx| {
        view.update(cx, |this, cx| {
            let st = this.state.borrow();
            let data = st.table_data_frames(frames[range.clone()].iter().copied());
            table_rows(&st, data, range.start, min_width, cx)
        })
        .unwrap_or_default()
    })
    .flex_1()
    .min_h_0()
    .min_w(min_width)
    .track_scroll(scroll);
    table
        .child(
            div()
                .relative()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .min_w(min_width)
                .overflow_hidden()
                .vertical_scrollbar(&scroll.0.borrow().base_handle)
                .child(body),
        )
        .into_any_element()
}

fn table_rows(
    st: &DocState,
    data: TableData,
    offset: usize,
    min_width: Pixels,
    cx: &mut Context<Probe>,
) -> Vec<AnyElement> {
    #[cfg(test)]
    RENDERED_ROWS.with(|count| count.set(count.get() + data.rows.len()));
    let mut rows = Vec::with_capacity(data.rows.len());
    for (local_index, values) in data.rows.into_iter().enumerate() {
        let row_index = offset + local_index;
        let current = data.frames[local_index] == st.frame;
        let mut row = div()
            .h(px(28.))
            .flex_none()
            .flex()
            .w_full()
            .min_w(min_width)
            .text_xs();
        for (col, value) in values.into_iter().enumerate() {
            let cell = Cell {
                row: row_index,
                col,
            };
            let selected = st.table_selection.contains(cell);
            row = row.child(
                div()
                    .id(("table-cell", row_index * data.headers.len().max(1) + col))
                    .flex_1()
                    .min_w_0()
                    .px_2()
                    .py_1()
                    .truncate()
                    .cursor_crosshair()
                    .bg(c(if selected {
                        0x1b2b45
                    } else if current {
                        0x16233a
                    } else {
                        0x0d1420
                    }))
                    .border_1()
                    .border_color(c(if selected { 0x4dabf7 } else { 0x1d2a3d }))
                    .text_color(c(if selected || current {
                        0xdbe6f5
                    } else {
                        0xa9bad2
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                            window.focus(&this.table_focus, cx);
                            let mut st = this.state.borrow_mut();
                            st.sync_table_selection();
                            st.table_selection.begin(
                                cell,
                                ev.modifiers.shift,
                                ev.modifiers.control || ev.modifiers.platform,
                            );
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _, cx| {
                        let mut st = this.state.borrow_mut();
                        if ev.pressed_button == Some(MouseButton::Left)
                            && st.table_selection.drag.is_some()
                            && st.table_selection.cursor != cell
                        {
                            st.table_selection.extend(cell);
                            cx.notify();
                        }
                    }))
                    .child(value),
            );
        }
        rows.push(row.into_any_element());
    }
    rows
}

#[cfg(test)]
std::thread_local! { pub(crate) static RENDERED_ROWS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

#[cfg(test)]
mod tests {
    use super::{Cell, TableData, TableSelection};
    use core::prelude::v1::test;

    #[test]
    fn shift_selects_intervals_but_control_toggles_disjoint_axes() {
        let mut s = TableSelection::default();
        s.begin(Cell { row: 0, col: 0 }, false, false);
        s.begin(Cell { row: 2, col: 2 }, false, true);
        assert_eq!(s.selected_rows(), 2);
        for row in [0, 2] {
            for col in [0, 2] {
                assert!(s.contains(Cell { row, col }));
            }
        }
        assert!(!s.contains(Cell { row: 1, col: 1 }));
        assert!(!s.contains(Cell { row: 0, col: 1 }));
        s.begin(Cell { row: 2, col: 2 }, false, true);
        assert_eq!(s.selected_rows(), 1);
        assert!(s.contains(Cell { row: 0, col: 0 }));
        assert!(!s.contains(Cell { row: 2, col: 2 }));
        s.begin(Cell { row: 2, col: 2 }, false, false);
        s.begin(Cell { row: 0, col: 0 }, true, false);
        assert_eq!(s.selected_rows(), 3);
        for row in 0..=2 {
            for col in 0..=2 {
                assert!(s.contains(Cell { row, col }));
            }
        }
        // Shift keeps the anchor, so extending back inward shrinks the range.
        s.begin(Cell { row: 1, col: 1 }, true, false);
        assert_eq!(s.selected_rows(), 2);
        assert!(!s.contains(Cell { row: 0, col: 0 }));
    }

    #[test]
    fn control_drag_uses_original_axes_without_accumulating_stale_cells() {
        let mut s = TableSelection::default();
        s.begin(Cell { row: 0, col: 0 }, false, false);
        s.begin(Cell { row: 2, col: 2 }, false, true);
        s.extend(Cell { row: 4, col: 4 });
        assert_eq!(s.selected_rows(), 4);
        assert!(!s.contains(Cell { row: 1, col: 1 }));
        assert!(s.contains(Cell { row: 0, col: 4 }));
        s.extend(Cell { row: 3, col: 3 });
        assert!(!s.contains(Cell { row: 4, col: 4 }));
        s.begin(Cell { row: 3, col: 3 }, false, true);
        s.extend(Cell { row: 2, col: 2 });
        assert_eq!(s.selected_rows(), 1);
        assert!(s.contains(Cell { row: 0, col: 0 }));
    }

    #[test]
    fn column_headers_toggle_columns_and_preserve_selected_rows() {
        let mut s = TableSelection::default();
        s.select_column(0, 5, false, false);
        assert_eq!(s.selected_rows(), 5);
        s.select_column(2, 5, false, true);
        assert!(s.contains(Cell { row: 3, col: 2 }));
        assert!(!s.contains(Cell { row: 3, col: 1 }));
        s.select_column(2, 5, false, true);
        assert!(!s.contains(Cell { row: 3, col: 2 }));
        s.select_column(0, 5, true, false);
        assert!(s.contains(Cell { row: 3, col: 1 }));
        s.begin(Cell { row: 1, col: 0 }, false, false);
        s.begin(Cell { row: 3, col: 0 }, false, true);
        s.select_column(2, 5, false, true);
        assert_eq!(s.selected_rows(), 2);
        assert!(!s.contains(Cell { row: 2, col: 2 }));
    }

    #[test]
    fn keyboard_and_context_changes() {
        let mut s = TableSelection::default();
        s.sync(1, vec![crate::plot::PlotVar::X; 5], vec![2, 7, 20]);
        s.begin(Cell { row: 0, col: 1 }, false, false);
        s.navigate("down", true, true, 3, 5);
        assert_eq!(s.selected_rows(), 3);
        s.sync(1, vec![crate::plot::PlotVar::X; 5], vec![2, 7, 20]);
        assert_eq!(s.selected_rows(), 3);
        s.sync(2, vec![crate::plot::PlotVar::X; 5], vec![2, 7, 20]);
        assert_eq!(s.selected_rows(), 0);
        s.select_all(0, 5);
        assert_eq!(s.selected_rows(), 0);
        s.select_all(3, 5);
        s.sync(2, vec![crate::plot::PlotVar::X; 3], vec![2, 7, 20]);
        assert_eq!(s.selected_rows(), 0);
        s.select_all(3, 3);
        s.sync(2, vec![crate::plot::PlotVar::X; 3], vec![2, 20]);
        assert_eq!(s.selected_rows(), 0);
        s.begin(Cell { row: 1, col: 1 }, true, false);
        assert_eq!(s.selected_rows(), 2);
        assert!(!s.contains(Cell { row: 2, col: 1 }));
    }

    #[test]
    fn csv_exports_only_selected_columns_and_rows_and_escapes_units() {
        let data = TableData {
            headers: vec!["t".into(), "x (a,\"b\")".into(), "y".into()],
            frames: vec![2, 7, 20],
            rows: vec![
                vec!["0".into(), "1".into(), "2".into()],
                vec!["3".into(), "4".into(), "5".into()],
                vec!["6".into(), "7".into(), "8".into()],
            ],
        };
        let mut s = TableSelection::default();
        s.begin(Cell { row: 1, col: 1 }, false, false);
        s.extend(Cell { row: 2, col: 2 });
        assert_eq!(data.csv(&s), "\"x (a,\"\"b\"\")\",y\r\n4,5\r\n7,8\r\n");
        s.clear();
        assert!(data.csv(&s).ends_with("6,7,8\r\n"));
        s.begin(Cell { row: 0, col: 0 }, false, false);
        s.begin(Cell { row: 2, col: 2 }, false, true);
        assert_eq!(data.csv(&s), "t,y\r\n0,2\r\n6,8\r\n");
    }

    #[test]
    fn excel_clipboard_uses_tabs_and_keeps_the_selected_matrix() {
        use crate::prefs::TableCopyFormat;
        let data = TableData {
            headers: vec!["t (s)".into(), "x (m)".into(), "y (m)".into()],
            frames: vec![0, 1, 2],
            rows: vec![
                vec!["0.000".into(), "1.234".into(), "-5.678".into()],
                vec!["0.033".into(), "2.345".into(), "-6.789".into()],
                vec!["0.067".into(), "3.456".into(), "-7.890".into()],
            ],
        };
        let mut s = TableSelection::default();
        s.begin(Cell { row: 0, col: 0 }, false, false);
        s.begin(Cell { row: 2, col: 2 }, false, true);
        assert_eq!(
            data.delimited(&s, TableCopyFormat::Excel.delimiter()),
            "t (s)\ty (m)\r\n0.000\t-5.678\r\n0.067\t-7.890\r\n"
        );
        assert_eq!(
            data.delimited(&s, TableCopyFormat::Csv.delimiter()),
            "t (s),y (m)\r\n0.000,-5.678\r\n0.067,-7.890\r\n"
        );
        let special = TableData {
            headers: vec!["x (a\tb)".into()],
            frames: vec![0],
            rows: vec![vec!["-".into()]],
        };
        assert_eq!(
            special.delimited(&TableSelection::default(), '\t'),
            "\"x (a\tb)\"\r\n-\r\n"
        );
    }

    #[test]
    fn sparse_frames_use_display_order_and_hidden_columns_do_not_leak_into_csv() {
        let mut doc = crate::DocState::placeholder();
        let mut track = crate::Track::point_mass(1, "Track", 0);
        track.set_point(2, 10., 20.);
        track.set_point(7, 30., 40.);
        track.set_point(20, 50., 60.);
        doc.tracks.push(track);
        doc.table_columns = [false, true, true, false, false];
        doc.sync_table_selection();
        let data = doc.table_data();
        assert_eq!(data.frames, vec![2, 7, 20]);
        doc.table_selection
            .begin(Cell { row: 1, col: 0 }, false, false);
        doc.table_selection.extend(Cell { row: 2, col: 1 });
        let expected = format!(
            "{},{}\r\n{},{}\r\n{},{}\r\n",
            data.headers[0],
            data.headers[1],
            data.rows[1][0],
            data.rows[1][1],
            data.rows[2][0],
            data.rows[2][1]
        );
        assert_eq!(doc.table_csv(), expected);
        doc.table_columns = [false; 5];
        doc.sync_table_selection();
        assert!(doc.table_csv().is_empty());
    }

    #[test]
    fn derived_columns_export_units_values_and_follow_visibility() {
        let mut doc = crate::DocState::placeholder();
        let mut track = crate::Track::point_mass(1, "Track", 0);
        track.set_point(0, 3., 4.);
        let r = track.set_variable(None, "r", "sqrt(x^2+y^2)").unwrap();
        track.set_variable(None, "undefined", "1/0").unwrap();
        doc.tracks.push(track);
        doc.calibration.decimals = 2;
        doc.table_columns = [false; 5];
        doc.sync_table_selection();
        let data = doc.table_data();
        assert_eq!(
            data.headers,
            vec![
                format!("r ({})", doc.calibration.unit),
                "undefined".to_string()
            ]
        );
        assert_eq!(data.rows, vec![vec!["5.00".to_string(), "-".to_string()]]);
        assert_eq!(
            doc.table_variables(),
            vec![
                crate::plot::PlotVar::Custom(r),
                crate::plot::PlotVar::Custom(r + 1)
            ]
        );
        doc.select_all_table();
        doc.tracks[0].variables[0].table_visible = false;
        doc.sync_table_selection();
        assert_eq!(doc.table_selection.selected_rows(), 0);
        assert_eq!(doc.table_csv(), "undefined\r\n-\r\n");
    }
    #[test]
    fn visible_ranges_match_full_export_with_sparse_frames_and_custom_variables() {
        let mut doc = crate::DocState::placeholder();
        let mut track = crate::Track::point_mass(1, "Track", 0);
        for frame in (0..20_000).step_by(2) {
            track.set_point(frame, frame as f32, 3.);
        }
        track.set_variable(None, "r", "sqrt(x^2+y^2)").unwrap();
        doc.tracks.push(track);
        let all = doc.table_data();
        let visible = doc.table_data_range(9_970..9_990);
        assert_eq!(visible.headers, all.headers);
        assert_eq!(visible.rows, all.rows[9_970..9_990]);
        assert_eq!(visible.frames, all.frames[9_970..9_990]);
        assert!(doc.table_data_range(10_000..10_020).rows.is_empty());
        assert!(doc.table_data_range(0..0).rows.is_empty());
        doc.tracks[0].set_point(19_940, 0., 4.);
        let updated = doc.table_data_range(9_970..9_971);
        assert_ne!(updated.rows[0], visible.rows[0]);
        doc.select_all_table();
        assert_eq!(doc.table_selection.selected_rows(), 10_000);
        assert_eq!(
            doc.table_data()
                .delimited(&doc.table_selection, '\t')
                .lines()
                .count(),
            10_001
        );
    }
}

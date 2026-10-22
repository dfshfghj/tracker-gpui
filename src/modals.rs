use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::*;

pub(crate) fn modal_button(
    label: &'static str,
    primary: bool,
    cx: &mut Context<Probe>,
    action: impl Fn(&mut Probe, &mut Context<Probe>) + 'static,
) -> AnyElement {
    div()
        .id(label)
        .px_3()
        .py_1()
        .rounded_sm()
        .cursor_pointer()
        .text_xs()
        .bg(if primary { c(0x4dabf7) } else { c(0x111a29) })
        .text_color(if primary { c(0x06101c) } else { c(0xdbe6f5) })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _ev, _window, cx| {
                action(this, cx);
                cx.notify();
            }),
        )
        .child(label)
        .into_any_element()
}

pub(crate) fn calibration_field_box(
    label: &'static str,
    value: String,
    active: bool,
    field: CalibrationField,
    state: Rc<RefCell<State>>,
    cx: &mut Context<Probe>,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(px(52.))
                .text_xs()
                .text_color(c(0x7d8ca6))
                .child(label),
        )
        .child(
            div()
                .id(label)
                .flex_1()
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(if active { c(0x16233a) } else { c(0x111a29) })
                .border_1()
                .border_color(if active { c(0x4dabf7) } else { c(0x1d2a3d) })
                .text_xs()
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |_this, _ev, _window, cx| {
                        state.borrow_mut().cal_field = field;
                        cx.notify();
                    }),
                )
                .child(format!("{value}{}", if active { "▏" } else { "" })),
        )
        .into_any_element()
}

pub(crate) fn calibration_modal_view(
    state: Rc<RefCell<State>>,
    cx: &mut Context<Probe>,
) -> AnyElement {
    let (length, unit, decimals, field, error, locked, active_stick, stick_count) = {
        let st = state.borrow();
        (
            st.cal_length_input.clone(),
            st.cal_unit_input.clone(),
            st.cal_decimals_input.clone(),
            st.cal_field,
            st.cal_error.clone(),
            st.coords.locked,
            st.active_stick,
            st.calibration.stick_count(),
        )
    };
    let description = format!(
        "编辑标尺 {} / {} 的长度。标尺只控制比例和角度，不改变坐标原点。",
        active_stick + 1,
        stick_count.max(1)
    );

    let rows = vec![
        calibration_field_box(
            "长度",
            length,
            field == CalibrationField::Length,
            CalibrationField::Length,
            state.clone(),
            cx,
        ),
        calibration_field_box(
            "单位",
            unit,
            field == CalibrationField::Unit,
            CalibrationField::Unit,
            state.clone(),
            cx,
        ),
        calibration_field_box(
            "小数位",
            decimals,
            field == CalibrationField::Decimals,
            CalibrationField::Decimals,
            state.clone(),
            cx,
        ),
    ];

    let apply_state = state.clone();
    let cancel_state = state.clone();
    crate::overlay::overlay_surface()
        .absolute()
        .top(px(0.))
        .left(px(0.))
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(c(0x000000).alpha(0.55))
        .child(
            div()
                .w(px(480.))
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .rounded_md()
                .bg(c(0x0d1420))
                .border_1()
                .border_color(c(0x2a3b55))
                .text_color(c(0xdbe6f5))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .child("标定设置"),
                )
                .child(div().text_xs().text_color(c(0x7d8ca6)).child(description))
                .children(rows)
                .when(locked, |this| {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(c(0xffd43b))
                            .child("坐标已锁定，修改不会生效。"),
                    )
                })
                .when_some(error, |this, error| {
                    this.child(div().text_xs().text_color(c(0xff6b6b)).child(error))
                })
                .child(
                    div()
                        .text_xs()
                        .text_color(c(0x5f6f88))
                        .child("Tab 切换字段 · Enter 应用 · Esc 取消"),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(modal_button("应用", true, cx, move |_this, _cx| {
                            apply_state.borrow_mut().apply_calibration_dialog();
                        }))
                        .child(modal_button("取消", false, cx, move |_this, _cx| {
                            let mut st = cancel_state.borrow_mut();
                            st.modal = None;
                            st.cal_error = None;
                        })),
                ),
        )
        .into_any_element()
}

pub(crate) fn operations_modal_view(
    state: Rc<RefCell<State>>,
    cx: &mut Context<Probe>,
) -> AnyElement {
    let rows = [
        ("播放 / 暂停", "空格"),
        ("逐帧", "← / →，Shift 为 10 帧"),
        (
            "移动 / 打点",
            "工具栏移动按钮或 T/Esc 清除工具选择；任意工具下 Shift+单击放置点",
        ),
        ("选择对象", "点击坐标轴/标尺/测量尺选中；拖动平移，端点调整"),
        (
            "对象属性",
            "布尔项用复选框；原点/角度/长度点击编辑，Enter 提交，Esc 取消",
        ),
        (
            "标定 / 测量",
            "工具栏选择工具；点击工具栏标尺按钮新建定标尺",
        ),
        (
            "质点轨道",
            "工具栏质点按钮列出所有质点轨道，点击切换当前打点目标",
        ),
        ("自动步进", "A 切换；新建点后前进 step size 帧"),
        (
            "轨迹显示",
            "默认最近 15 帧长轨迹并显示帧号；按住 Shift 只显示当前帧；View 可切换轨迹/步号/轨迹线",
        ),
        (
            "AutoTracker",
            "工具栏或 Tracks 菜单打开独立窗口并置顶于主窗口；关键帧显示目标十字/实线掩膜，非关键帧显示点线匹配，搜索框为虚线",
        ),
        (
            "跟踪修正",
            "暂停或停止后在当前帧 Shift+单击修正，再点搜索从该帧继续；可累计多个关键帧",
        ),
        (
            "右侧页签",
            "顶部切换“对象属性”和“绘图+数据”；绘图/数据仅用于质点轨道",
        ),
        (
            "绘图",
            "始终自动缩放，点数据点跳帧；轴标签在轴旁，无 vs 标题",
        ),
        (
            "数据表",
            "列复选框默认 t/x/y；点击行选择，全选/清除/复制或 Ctrl+C",
        ),
        (
            "删除对象",
            "Delete / Backspace；未选中对象时删除当前帧点，坐标轴不可删",
        ),
        ("撤销 / 重做", "Ctrl+Z / Ctrl+Shift+Z"),
        ("缩放 / 平移", "滚轮缩放；Ctrl/Alt+滚轮平移；F 适应窗口"),
        ("标签页", "左侧点击视频打开，× 关闭；标签栏滚轮左右滚动"),
        (
            "侧栏",
            "标题栏箭头收起；折叠窄条展开；View 菜单可切换左右侧栏",
        ),
    ];
    let close_state = state.clone();
    let mut body = div().flex().flex_col().gap_2();
    for (action, keys) in rows {
        body = body.child(
            div()
                .flex()
                .gap_3()
                .text_xs()
                .child(
                    div()
                        .w(px(108.))
                        .flex_none()
                        .text_color(c(0xdbe6f5))
                        .child(action),
                )
                .child(div().flex_1().text_color(c(0x9fb2cc)).child(keys)),
        );
    }
    crate::overlay::overlay_surface()
        .absolute()
        .top(px(0.))
        .left(px(0.))
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(c(0x000000).alpha(0.55))
        .child(
            div()
                .w(px(450.))
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .rounded_md()
                .bg(c(0x0d1420))
                .border_1()
                .border_color(c(0x2a3b55))
                .text_color(c(0xdbe6f5))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .child("操作说明"),
                )
                .child(body)
                .child(div().flex().justify_end().child(modal_button(
                    "关闭",
                    true,
                    cx,
                    move |_this, _cx| {
                        close_state.borrow_mut().modal = None;
                    },
                ))),
        )
        .into_any_element()
}

pub(crate) fn modal_view(state: Rc<RefCell<State>>, cx: &mut Context<Probe>) -> AnyElement {
    let Some(modal) = state.borrow().modal.clone() else {
        return div().into_any_element();
    };
    if matches!(modal, Modal::Calibration) {
        return calibration_modal_view(state, cx);
    }
    if matches!(modal, Modal::Operations) {
        return operations_modal_view(state, cx);
    }

    let mut buttons = Vec::new();
    let (title, message): (&str, String) = match &modal {
        Modal::CloseDirty { index } => {
            let name = state
                .borrow()
                .docs
                .get(*index)
                .map(|doc| doc.title.clone())
                .unwrap_or_else(|| "视频".into());
            let index = *index;
            buttons.push(modal_button("保存", true, cx, move |this, cx| {
                request_save(this.state.clone(), SaveAction::CloseDoc(index), cx);
            }));
            buttons.push(modal_button("放弃", false, cx, move |this, _cx| {
                let mut st = this.state.borrow_mut();
                st.close_doc(index);
                st.modal = None;
            }));
            buttons.push(modal_button("取消", false, cx, move |this, _cx| {
                this.state.borrow_mut().modal = None;
            }));
            ("未保存的修改", format!("“{name}” 有未保存的修改。"))
        }
        Modal::ConfirmNew => {
            buttons.push(modal_button(
                "保存并新建",
                true,
                cx,
                move |this, cx| {
                    request_save(this.state.clone(), SaveAction::NewProject, cx);
                },
            ));
            buttons.push(modal_button(
                "放弃并新建",
                false,
                cx,
                move |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.new_project();
                    st.modal = None;
                },
            ));
            buttons.push(modal_button("取消", false, cx, move |this, _cx| {
                this.state.borrow_mut().modal = None;
            }));
            (
                "新建项目",
                "当前项目有未保存的修改，是否先保存？".to_string(),
            )
        }
        Modal::ConfirmOpenProject(path) => {
            let path_for_save = path.clone();
            buttons.push(modal_button(
                "保存并打开",
                true,
                cx,
                move |this, cx| {
                    request_save(
                        this.state.clone(),
                        SaveAction::OpenProject(path_for_save.clone()),
                        cx,
                    );
                },
            ));
            let path_for_discard = path.clone();
            buttons.push(modal_button(
                "放弃并打开",
                false,
                cx,
                move |this, cx| {
                    open_project_file(this.state.clone(), path_for_discard.clone(), cx);
                },
            ));
            buttons.push(modal_button("取消", false, cx, move |this, _cx| {
                this.state.borrow_mut().modal = None;
            }));
            (
                "打开项目",
                "当前项目有未保存的修改，是否先保存？".to_string(),
            )
        }
        Modal::About => {
            buttons.push(modal_button("关闭", true, cx, move |this, _cx| {
                this.state.borrow_mut().modal = None;
            }));
            (
                "关于 Tracker GPUI",
                "Tracker GPUI · FFmpeg 9 · gpui-kit 0.7.3".to_string(),
            )
        }
        Modal::Error(message) => {
            buttons.push(modal_button("关闭", true, cx, move |this, _cx| {
                this.state.borrow_mut().modal = None;
            }));
            ("错误", message.clone())
        }
        Modal::Calibration | Modal::Operations => unreachable!(),
    };

    crate::overlay::overlay_surface()
        .absolute()
        .top(px(0.))
        .left(px(0.))
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(c(0x000000).alpha(0.55))
        .child(
            div()
                .w(px(460.))
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .rounded_md()
                .bg(c(0x0d1420))
                .border_1()
                .border_color(c(0x2a3b55))
                .text_color(c(0xdbe6f5))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .child(title.to_string()),
                )
                .child(div().text_xs().text_color(c(0x9fb2cc)).child(message))
                .child(div().flex().justify_end().gap_2().children(buttons)),
        )
        .into_any_element()
}

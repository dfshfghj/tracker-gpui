use crate::{
    export_overlay::{Renderer, Scene},
    export_video::{self, Options, Progress},
    video::FrameSource,
    *,
};
use gpui_component::{
    input::{InputEvent, InputState},
    select::{Select, SelectEvent, SelectItem, SelectState},
    slider::{Slider, SliderEvent, SliderState, SliderValue},
    FocusableExt, Sizable,
};
use std::{sync::atomic::Ordering, time::Duration};

#[derive(Clone)]
struct Choice {
    value: String,
    label: String,
}
impl SelectItem for Choice {
    type Value = String;
    fn title(&self) -> SharedString {
        self.label.clone().into()
    }
    fn value(&self) -> &String {
        &self.value
    }
}
type Choices = SelectState<Vec<Choice>>;
pub(crate) struct ExportWindow {
    source: FrameSource,
    scene: Arc<Scene>,
    first: Entity<InputState>,
    last: Entity<InputState>,
    frame: Entity<InputState>,
    frame_slider: Entity<SliderState>,
    range_slider: Entity<SliderState>,
    scale: Entity<Choices>,
    fps: Entity<Choices>,
    bitrate: Entity<Choices>,
    audio: bool,
    preview: Option<Arc<RenderImage>>,
    retired: Vec<Arc<RenderImage>>,
    generation: u64,
    preview_busy: bool,
    choosing: bool,
    progress: Option<Arc<Progress>>,
    message: String,
    _window_closed: Subscription,
    closed: bool,
}
impl Drop for ExportWindow {
    fn drop(&mut self) {
        if let Some(progress) = &self.progress {
            progress.cancel.store(true, Ordering::Relaxed);
        }
    }
}
impl Probe {
    pub(crate) fn open_export_window(&mut self, cx: &mut Context<Self>) {
        let (source, scene, frame) = {
            let state = self.state.borrow();
            let Some(video) = &state.video else {
                return;
            };
            (video.frame_source(), Scene::capture(&state), state.frame)
        };
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(150.), px(70.)),
                size: size(px(920.), px(760.)),
            })),
            titlebar: Some(TitlebarOptions {
                title: Some(crate::i18n::tr("export.title").into()),
                ..Default::default()
            }),
            is_resizable: true,
            focus: true,
            show: true,
            ..Default::default()
        };
        if let Err(error) = gpui_kit::open_window(options, cx, move |window, cx| {
            cx.new(|cx| ExportWindow::new(source, scene, frame, window, cx))
        }) {
            self.state.borrow_mut().status = Some(error.to_string());
        }
    }
}
impl ExportWindow {
    fn new(
        source: FrameSource,
        scene: Scene,
        frame: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let owner = cx.entity().downgrade();
        let id = window.window_handle().window_id();
        let window_closed = cx.on_window_closed(move |cx, closed| {
            if closed == id {
                let _ = owner.update(cx, |this: &mut Self, _| {
                    this.closed = true;
                    this.generation += 1;
                    if let Some(progress) = &this.progress {
                        progress.cancel.store(true, Ordering::Relaxed);
                    }
                });
            }
        });
        let input = |text: String, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| {
                let mut input = InputState::new(window, cx);
                input.set_value(text, window, cx);
                input
            })
        };
        let first = input("0".into(), window, cx);
        let last = input(
            source.meta().frames.saturating_sub(1).to_string(),
            window,
            cx,
        );
        let frame = input(frame.to_string(), window, cx);
        let maximum = source.meta().frames.saturating_sub(1) as f32;
        let current_frame = frame.read(cx).value().parse::<f32>().unwrap_or(0.);
        let frame_slider = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(maximum.max(1.))
                .step(1.)
                .default_value(current_frame)
        });
        let range_slider = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(maximum.max(1.))
                .step(1.)
                .default_value(SliderValue::Range(0., maximum))
        });
        cx.subscribe_in(
            &frame_slider,
            window,
            |this, _, event: &SliderEvent, window, cx| {
                if this.progress.is_some() || this.choosing {
                    return;
                }
                if let SliderEvent::Change(value) = event {
                    this.frame.update(cx, |input, cx| {
                        input.set_value((value.start().round() as usize).to_string(), window, cx)
                    });
                    this.sync_frame_controls(window, cx);
                    this.request_preview(cx);
                }
            },
        )
        .detach();
        cx.subscribe_in(
            &range_slider,
            window,
            |this, _, event: &SliderEvent, window, cx| {
                if this.progress.is_some() || this.choosing {
                    return;
                }
                if let SliderEvent::Change(value) = event {
                    this.first.update(cx, |input, cx| {
                        input.set_value((value.start().round() as usize).to_string(), window, cx)
                    });
                    this.last.update(cx, |input, cx| {
                        input.set_value((value.end().round() as usize).to_string(), window, cx)
                    });
                    this.sync_frame_controls(window, cx);
                    this.request_preview(cx);
                }
            },
        )
        .detach();
        let scale = Self::choices(
            vec![
                ("1", crate::i18n::tr("export.original")),
                ("0.75", "75%".into()),
                ("0.5", "50%".into()),
                ("0.25", "25%".into()),
            ],
            "1",
            window,
            cx,
        );
        let fps = Self::choices(
            vec![
                ("source", crate::i18n::tr("export.original")),
                ("24", "24 fps".into()),
                ("30", "30 fps".into()),
                ("60", "60 fps".into()),
            ],
            "source",
            window,
            cx,
        );
        let bitrate = Self::choices(
            vec![
                ("2000000", "2 Mbps".into()),
                ("5000000", "5 Mbps".into()),
                ("10000000", "10 Mbps".into()),
                ("20000000", "20 Mbps".into()),
            ],
            "10000000",
            window,
            cx,
        );
        for field in [&first, &last, &frame] {
            cx.subscribe_in(field, window, |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur)
                    && this.progress.is_none()
                {
                    this.sync_frame_controls(window, cx);
                    this.request_preview(cx);
                }
            })
            .detach();
        }
        for select in [&scale, &fps, &bitrate] {
            cx.subscribe(select, |this, _, event: &SelectEvent<Vec<Choice>>, cx| {
                if matches!(event, SelectEvent::Confirm(_)) {
                    this.request_preview(cx);
                }
            })
            .detach();
        }
        let mut view = Self {
            source,
            scene: Arc::new(scene),
            first,
            last,
            frame,
            frame_slider,
            range_slider,
            scale,
            fps,
            bitrate,
            audio: true,
            preview: None,
            retired: Vec::new(),
            generation: 0,
            preview_busy: false,
            choosing: false,
            progress: None,
            message: String::new(),
            _window_closed: window_closed,
            closed: false,
        };
        view.request_preview(cx);
        view
    }
    fn choices(
        items: Vec<(&str, String)>,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<Choices> {
        cx.new(|cx| {
            let mut state = SelectState::new(
                items
                    .into_iter()
                    .map(|(v, label)| Choice {
                        value: v.into(),
                        label,
                    })
                    .collect::<Vec<_>>(),
                None,
                window,
                cx,
            );
            state.set_selected_value(&value.to_string(), window, cx);
            state
        })
    }
    fn value(select: &Entity<Choices>, cx: &App) -> String {
        select
            .read(cx)
            .selected_value()
            .cloned()
            .unwrap_or_default()
    }
    fn options(&self, cx: &App) -> Result<Options, String> {
        let parse = |field: &Entity<InputState>| {
            field
                .read(cx)
                .value()
                .trim()
                .parse::<usize>()
                .map_err(|_| crate::i18n::tr("export.invalid_range"))
        };
        let options = Options {
            first: parse(&self.first)?,
            last: parse(&self.last)?,
            scale: Self::value(&self.scale, cx)
                .parse()
                .map_err(|_| "invalid scale")?,
            fps: Self::value(&self.fps, cx).parse().ok(),
            bitrate: Self::value(&self.bitrate, cx)
                .parse()
                .map_err(|_| "invalid bitrate")?,
            audio: self.audio,
        };
        options
            .validate(&self.source)
            .map_err(|_| crate::i18n::tr("export.invalid_range"))?;
        Ok(options)
    }
    fn sync_frame_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Ok(options) = self.options(cx) else {
            return;
        };
        let frame = self
            .frame
            .read(cx)
            .value()
            .trim()
            .parse::<usize>()
            .unwrap_or(options.first)
            .clamp(options.first, options.last);
        self.frame.update(cx, |input, cx| {
            input.set_value(frame.to_string(), window, cx)
        });
        self.frame_slider
            .update(cx, |slider, cx| slider.set_value(frame as f32, window, cx));
        self.range_slider.update(cx, |slider, cx| {
            slider.set_value(
                SliderValue::Range(options.first as f32, options.last as f32),
                window,
                cx,
            )
        });
    }
    fn request_preview(&mut self, cx: &mut Context<Self>) {
        if self.closed || self.progress.is_some() {
            return;
        }
        if self.preview_busy {
            self.generation += 1;
            return;
        }
        let options = match self.options(cx) {
            Ok(options) => options,
            Err(error) => {
                self.message = error;
                cx.notify();
                return;
            }
        };
        let frame = self
            .frame
            .read(cx)
            .value()
            .trim()
            .parse::<usize>()
            .unwrap_or(options.first)
            .clamp(options.first, options.last);
        self.generation += 1;
        let generation = self.generation;
        self.preview_busy = true;
        self.message.clear();
        let source = self.source.clone();
        let scene = self.scene.clone();
        let dimensions = options.dimensions(&source);
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let data = source.reader()?.read(frame)?;
                    Renderer::new().frame(&scene, &data, dimensions)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.generation {
                    this.preview_busy = false;
                    this.request_preview(cx);
                    return;
                }
                this.preview_busy = false;
                match result {
                    Ok(frame) => {
                        if let Some(old) =
                            this.preview
                                .replace(make_image(frame.bgra, frame.width, frame.height))
                        {
                            this.retired.push(old);
                        }
                    }
                    Err(error) => this.message = error,
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn choose_destination(&mut self, cx: &mut Context<Self>) {
        let options = match self.options(cx) {
            Ok(o) => o,
            Err(e) => {
                self.message = e;
                cx.notify();
                return;
            }
        };
        if self.choosing || self.progress.is_some() {
            return;
        }
        self.choosing = true;
        let directory = self
            .source
            .meta()
            .path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();
        let name = format!(
            "{}-annotated.mp4",
            self.source
                .meta()
                .path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
        );
        let receiver = cx.prompt_for_new_path(&directory, Some(&name));
        cx.spawn(async move |this, cx| {
            let result = receiver.await;
            let _ = this.update(cx, |this, cx| {
                this.choosing = false;
                match result {
                    Ok(Ok(Some(path)))
                        if path
                            .extension()
                            .and_then(|s| s.to_str())
                            .is_some_and(|s| s.eq_ignore_ascii_case("mp4")) =>
                    {
                        this.start(options, path, cx)
                    }
                    Ok(Ok(Some(_))) => this.message = crate::i18n::tr("export.mp4_name"),
                    Ok(Err(error)) => this.message = error.to_string(),
                    _ => {}
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn start(&mut self, options: Options, path: PathBuf, cx: &mut Context<Self>) {
        if self.closed {
            return;
        }
        let progress = Arc::new(Progress::default());
        self.progress = Some(progress.clone());
        self.message.clear();
        let source = self.source.clone();
        let scene = self.scene.clone();
        let destination = path.clone();
        let job = progress.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        if let Err(error) = std::thread::Builder::new()
            .name("video-export".into())
            .spawn(move || {
                let result = export_video::export(&source, &scene, &options, &destination, &job);
                let _ = tx.send(result);
            })
        {
            self.progress = None;
            self.message = error.to_string();
            return;
        }
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            match rx.try_recv() {
                Ok(result) => {
                    let _ = this.update(cx, |this, cx| {
                        this.progress = None;
                        this.message = match result {
                            Ok(()) => {
                                format!("{} {}", crate::i18n::tr("export.complete"), path.display())
                            }
                            Err(error) if error == "cancelled" => {
                                crate::i18n::tr("export.cancelled")
                            }
                            Err(error) => error,
                        };
                        cx.notify();
                    });
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    let _ = this.update(cx, |this, cx| {
                        this.progress = None;
                        this.message = crate::i18n::tr("export.failed");
                        cx.notify();
                    });
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    if this.update(cx, |_, cx| cx.notify()).is_err() {
                        progress.cancel.store(true, Ordering::Relaxed);
                        break;
                    }
                }
            }
        })
        .detach();
        cx.notify();
    }
    fn button(
        &self,
        id: &'static str,
        label: String,
        enabled: bool,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        div()
            .id(id)
            .px_3()
            .py_1()
            .rounded_sm()
            .bg(c(0x152238))
            .text_color(c(if enabled { 0x9fc9f5 } else { 0x5f6f88 }))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    if enabled {
                        action(this, window, cx);
                        cx.stop_propagation();
                    }
                }),
            )
            .child(label)
            .into_any_element()
    }
}
fn row(label: String, control: AnyElement) -> AnyElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_xs().text_color(c(0x8fa3bf)).child(label))
        .child(control)
        .into_any_element()
}
fn select_control(state: &Entity<Choices>, disabled: bool) -> AnyElement {
    // Keep the regular border identical when focused or open. The library's
    // appearance mode otherwise paints a separate active border after styles.
    Select::new(state)
        .small()
        .w_full()
        .disabled(disabled)
        .appearance(false)
        .focus_ring(false)
        .bg(c(0x111a29))
        .text_color(c(0xdbe6f5))
        .border_1()
        .border_color(c(0x1d2a3d))
        .rounded_sm()
        .into_any_element()
}
impl Render for ExportWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for image in self.retired.drain(..) {
            let _ = window.drop_image(image);
        }
        let busy = self.progress.is_some() || self.choosing;
        let single = self.source.meta().frames <= 1;
        let dimensions = self.options(cx).ok().map(|o| o.dimensions(&self.source));
        let mut preview = div()
            .relative()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .bg(c(0x000000))
            .overflow_hidden();
        if let Some(image) = &self.preview {
            preview = preview.child(
                div().absolute().inset_0().child(
                    img(image.clone())
                        .size_full()
                        .object_fit(ObjectFit::Contain),
                ),
            );
        }
        let left = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .child(preview)
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .child(crate::i18n::tr("export.preview_frame")),
                    )
                    .child(
                        div()
                            .w(px(85.))
                            .flex_none()
                            .child(crate::widgets::property_input(
                                "export-preview-frame",
                                &self.frame,
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Slider::new(&self.frame_slider).disabled(busy || single)),
                    ),
            )
            .child(div().flex_none().text_xs().text_color(c(0x8fa3bf)).child(
                if self.preview_busy {
                    crate::i18n::tr("export.loading")
                } else {
                    dimensions
                        .map(|(w, h)| format!("{w} × {h} · MP4 / MPEG-4"))
                        .unwrap_or_default()
                },
            ));
        let range = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(Slider::new(&self.range_slider).disabled(busy || single))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(crate::widgets::property_input(
                                "export-first",
                                &self.first,
                                cx,
                            )),
                    )
                    .child("—")
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(crate::widgets::property_input(
                                "export-last",
                                &self.last,
                                cx,
                            )),
                    ),
            );
        let mut parameters = div()
            .w_full()
            .flex_none()
            .p_4()
            .flex()
            .flex_col()
            .gap_5()
            .child(row(
                crate::i18n::tr("export.range"),
                range.into_any_element(),
            ))
            .child(row(
                crate::i18n::tr("export.resolution"),
                select_control(&self.scale, busy),
            ))
            .child(row(
                crate::i18n::tr("export.fps"),
                select_control(&self.fps, busy),
            ))
            .child(row(
                crate::i18n::tr("export.bitrate"),
                select_control(&self.bitrate, busy),
            ))
            .child(row(
                crate::i18n::tr("export.audio"),
                self.button(
                    "export-audio",
                    crate::i18n::tr(if self.audio {
                        "export.audio_on"
                    } else {
                        "export.audio_off"
                    }),
                    !busy,
                    cx,
                    |this, _, cx| {
                        this.audio = !this.audio;
                        cx.notify();
                    },
                ),
            ));
        if let Some(progress) = &self.progress {
            let ratio = progress.done.load(Ordering::Relaxed) as f32
                / progress.total.load(Ordering::Relaxed).max(1) as f32;
            parameters = parameters
                .child(div().text_color(c(0x8ecbff)).child(format!(
                    "{} {:.0}%",
                    crate::i18n::tr("export.running"),
                    ratio * 100.
                )))
                .child(
                    div()
                        .w_full()
                        .h(px(4.))
                        .bg(c(0x152238))
                        .child(div().h_full().w(relative(ratio)).bg(c(0x4dabf7))),
                );
        }
        parameters = parameters.child(
            div()
                .text_xs()
                .text_color(c(0x9fc9f5))
                .child(self.message.clone()),
        );
        let action = if self.progress.is_some() {
            self.button(
                "cancel-export",
                crate::i18n::tr("export.cancel"),
                true,
                cx,
                |this, _, _| {
                    if let Some(progress) = &this.progress {
                        progress.cancel.store(true, Ordering::Relaxed);
                    }
                },
            )
        } else {
            self.button(
                "start-export",
                crate::i18n::tr("export.start"),
                !busy,
                cx,
                |this, _, cx| this.choose_destination(cx),
            )
        };
        div()
            .size_full()
            .flex()
            .bg(c(0x070b12))
            .text_color(c(0xdbe6f5))
            .text_sm()
            .child(left)
            .child(
                div()
                    .w(px(300.))
                    .max_w(relative(0.45))
                    .flex_none()
                    .min_h_0()
                    .border_l_1()
                    .border_color(c(0x1d2a3d))
                    .flex()
                    .flex_col()
                    .child(crate::scroll_area::vertical(
                        "export-parameters",
                        parameters,
                    ))
                    .child(
                        div()
                            .flex_none()
                            .p_3()
                            .border_t_1()
                            .border_color(c(0x1d2a3d))
                            .flex()
                            .justify_end()
                            .child(action),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::gpui;
    use core::prelude::v1::test;

    #[gpui::test]
    fn native_export_window_previews_and_validates_inputs_and_cancels_on_close(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_kit::init);
        let path =
            std::env::temp_dir().join(format!("tracker-export-window-{}.mp4", std::process::id()));
        std::fs::write(&path, include_bytes!("../tests/fixtures/export-source.mp4")).unwrap();
        let stream = crate::video::VideoStream::open(path.clone(), 0).unwrap();
        let source = stream.frame_source();
        let (handle, view) = cx
            .update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds {
                            origin: point(px(0.), px(0.)),
                            size: size(px(920.), px(760.)),
                        })),
                        ..Default::default()
                    },
                    cx,
                    |window, cx| {
                        cx.new(|cx| {
                            ExportWindow::new(
                                source,
                                Scene::capture(&DocState::placeholder()),
                                3,
                                window,
                                cx,
                            )
                        })
                    },
                )
            })
            .unwrap();
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            assert!(view.preview.is_some(), "preview failed: {}", view.message);
            assert!(!view.preview_busy);
            let options = view.options(cx).unwrap();
            assert_eq!((options.first, options.last), (0, 11));
            assert!(options.fps.is_none());
            assert!(options.audio);
        });
        handle
            .update(cx, |_, window, cx| {
                view.update(cx, |view, cx| {
                    view.first
                        .update(cx, |input, cx| input.set_value("8", window, cx));
                    view.last
                        .update(cx, |input, cx| input.set_value("2", window, cx));
                    assert!(view.options(cx).is_err());
                    view.first
                        .update(cx, |input, cx| input.set_value("0", window, cx));
                    view.last
                        .update(cx, |input, cx| input.set_value("11", window, cx));
                    view.scale.update(cx, |select, cx| {
                        select.set_selected_value(&"0.5".into(), window, cx)
                    });
                    view.request_preview(cx);
                });
                let _ = window.draw(cx);
            })
            .unwrap();
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            view.range_slider.update(cx, |_, cx| {
                cx.emit(SliderEvent::Change(SliderValue::Range(2., 7.)))
            });
        });
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            let options = view.options(cx).unwrap();
            assert_eq!((options.first, options.last), (2, 7));
            for frame in 0..12 {
                view.frame_slider.update(cx, |_, cx| {
                    cx.emit(SliderEvent::Change(SliderValue::Single(frame as f32)))
                });
            }
        });
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            assert_eq!(view.frame.read(cx).value().as_ref(), "7");
            assert_eq!(view.frame_slider.read(cx).value(), SliderValue::Single(7.));
            assert!(!view.preview_busy, "coalesced preview never completed");
        });
        handle
            .update(cx, |_, window, cx| {
                view.update(cx, |view, cx| {
                    view.frame.update(cx, |input, cx| {
                        input.set_value("4", window, cx);
                        cx.emit(InputEvent::Blur);
                    });
                });
            })
            .unwrap();
        cx.run_until_parked();
        handle
            .update(cx, |_, window, cx| {
                let _ = window.draw(cx);
            })
            .unwrap();
        view.update(cx, |view, cx| {
            assert_eq!(view.frame_slider.read(cx).value(), SliderValue::Single(4.));
            crate::widgets::PROPERTY_INPUT_BOUNDS.with(|map| {
                let map = map.borrow();
                let frame = map[&view.frame.entity_id()];
                let first = map[&view.first.entity_id()];
                assert!(
                    first.left() > frame.right(),
                    "parameters must be to the right of preview controls"
                );
                assert!(
                    frame.top() > first.bottom(),
                    "preview controls must sit below the video"
                );
                assert!(first.size.width > px(70.));
            });
        });
        let progress = Arc::new(Progress::default());
        view.update(cx, |view, cx| {
            assert!(view.preview.is_some());
            assert_eq!(view.options(cx).unwrap().dimensions(&view.source), (48, 32));
            view.progress = Some(progress.clone());
        });
        handle
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
        drop(view);
        cx.run_until_parked();
        assert!(progress.cancel.load(Ordering::Relaxed));
        drop(stream);
        std::fs::remove_file(path).unwrap();
    }
}
